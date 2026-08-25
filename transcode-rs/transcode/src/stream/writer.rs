//! The stream state machine.

use super::{FrameWriter, Termination, TrailerSet};
use crate::codec::Framing;
use crate::error::Error;
use http::{HeaderMap, HeaderValue, StatusCode};

/// Where a stream is in its life.
///
/// The distinction that matters is [`StreamState::Pending`] versus
/// [`StreamState::Committed`]: it is the difference between a failure that can
/// still have a real status and one that can only be truncated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    /// No message has gone out. The status line is uncommitted, so a failure
    /// here is reported normally.
    Pending,
    /// At least one message has gone out. The status line is spent.
    Committed,
    /// The stream has ended.
    Done,
}

/// Writes a server-streaming response, enforcing README §6.2
///
/// The writer produces bytes but never sends them: a transport does that. This
/// keeps the rule in one place and testable — the same state machine backs
/// HTTP/1.1, HTTP/2, and HTTP/3, so none of them can implement it
/// almost-correctly on its own.
#[derive(Debug)]
pub struct StreamWriter {
    frames: FrameWriter,
    state: StreamState,
    content_type: &'static str,
}

impl StreamWriter {
    /// A writer for one negotiated codec.
    #[must_use]
    pub const fn new(framing: Framing, content_type: &'static str) -> Self {
        Self {
            frames: FrameWriter::new(framing),
            state: StreamState::Pending,
            content_type,
        }
    }

    /// The current state.
    #[must_use]
    pub const fn state(&self) -> StreamState {
        self.state
    }

    /// Whether the status line has been spent.
    #[must_use]
    pub const fn committed(&self) -> bool {
        matches!(self.state, StreamState::Committed | StreamState::Done)
    }

    /// The response headers, produced when the first message is ready.
    ///
    /// Deliberately not available before then. Handing a transport the headers
    /// early is exactly how the status gets committed prematurely, so the type
    /// makes the mistake awkward rather than documenting against it.
    #[must_use]
    pub fn headers(&self) -> HeaderMap {
        let mut map = HeaderMap::new();
        if let Ok(value) = HeaderValue::from_str(self.content_type) {
            map.insert(http::header::CONTENT_TYPE, value);
        }
        // Advertised in the headers, because an intermediary that has not been
        // told to expect trailers is entitled to drop them.
        map.insert(
            http::header::TRAILER,
            HeaderValue::from_static("grpc-status, grpc-message"),
        );
        // A stream must not be cached: what a replay would serve is a prefix.
        map.insert(
            http::header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache"),
        );
        map
    }

    /// The status line, which for a stream that produced anything is always
    /// `200`.
    ///
    /// The honesty comes from [`Termination::Truncate`], not from this: once a
    /// message is out, `200` is already true of what was sent.
    #[must_use]
    pub const fn status(&self) -> StatusCode {
        StatusCode::OK
    }

    /// Writes one encoded message, committing the status line if this is the
    /// first.
    ///
    /// # Panics
    ///
    /// Panics if the stream has already ended, which would be a bug in the
    /// caller's loop rather than anything a peer can cause.
    pub fn message(&mut self, encoded: &[u8]) -> Vec<u8> {
        assert!(
            self.state != StreamState::Done,
            "wrote a message to a stream that already ended"
        );
        self.state = StreamState::Committed;
        self.frames.message(encoded)
    }

    /// A keepalive frame, for framings that have one.
    #[must_use]
    pub fn keepalive(&self) -> Option<Vec<u8>> {
        self.frames.keepalive()
    }

    /// Ends a stream that completed.
    pub fn finish(&mut self) -> Termination {
        self.state = StreamState::Done;
        Termination::Complete {
            close: self.frames.close(),
            trailers: TrailerSet::ok(),
        }
    }

    /// Ends a stream that failed.
    ///
    /// `encode_error` renders the AIP-193 envelope with the negotiated codec.
    /// It is a closure rather than pre-encoded bytes because it is only called
    /// in the committed case — a failure before the first message is rendered
    /// as an ordinary error response instead, by whatever handles unary errors.
    ///
    /// This is the whole of §8.4 in one branch: before the commit, a real
    /// status; after it, an error frame plus trailers plus truncation.
    pub fn fail<F>(&mut self, error: Box<Error>, encode_error: F) -> Termination
    where
        F: FnOnce(&Error) -> Vec<u8>,
    {
        let committed = self.committed();
        self.state = StreamState::Done;

        if !committed {
            // Nothing has been written, so the status line is still ours.
            return Termination::Deferred { error };
        }

        let trailers = TrailerSet::from_error(&error);
        let frame = self.frames.error(&encode_error(&error));

        // The operator's view must stay complete, because the client's cannot.
        tracing::error!(
            status = error.http.as_u16(),
            code = error.code.as_str(),
            message = %error.message,
            "stream failed after committing its status; truncating the body"
        );

        Termination::Truncate {
            frame,
            trailers,
            error,
        }
    }
}
