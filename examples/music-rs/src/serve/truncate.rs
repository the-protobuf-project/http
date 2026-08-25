//! Ending a response body abnormally.
//!
//! This is the transport half of README §6.2. Once a streaming response has
//! written its first message the status line is spent, so a failure after that
//! point cannot be reported by status: the only signal HTTP has left is to stop
//! short of a clean end. A listener that completes the body instead is telling
//! the client the stream succeeded.
//!
//! Truncation is what makes `curl` exit 18, `fetch()` reject, and a Go client
//! return `io.ErrUnexpectedEOF`.

use bytes::Bytes;
use http_body::{Body, Frame};
use std::pin::Pin;
use std::task::{Context, Poll};

/// The error a truncated body ends with.
///
/// Returning an error from a body is how a hyper-based listener is told to
/// terminate the response abnormally — `RST_STREAM` with `INTERNAL_ERROR` on
/// HTTP/2 and HTTP/3, and a close without the terminating zero-length chunk on
/// HTTP/1.1. Every listener in this example inherits that from its body type,
/// so none of them can implement the rule almost-correctly on its own.
#[derive(Debug)]
pub struct TruncatedStream;

impl std::fmt::Display for TruncatedStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("stream failed after committing its status; body truncated")
    }
}

impl std::error::Error for TruncatedStream {}

/// Where a truncating body is in its short life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// The frame has not been handed over yet.
    Pending,
    /// The frame has been handed over but may still be sitting in the
    /// listener's write buffer.
    Flushing,
    /// The failure has been reported.
    Failed,
}

/// A body that yields its bytes once and then fails.
///
/// The bytes go out first — they carry the in-band error frame, so a client
/// that does read the body learns why — and the failure that follows is what
/// truncates the response.
///
/// Between the two there is one deliberate yield back to the runtime. hyper
/// buffers the response head together with the first body frame and flushes
/// once; failing in the same pass would discard both, and the client would see
/// an empty reply rather than a truncated one — losing the status line, the
/// error frame, and every message the stream had already produced.
#[derive(Debug)]
pub struct Truncating {
    /// The frame still to be written, taken on the first poll.
    pending: Option<Bytes>,
    /// How far along the body is.
    stage: Stage,
}

impl Truncating {
    /// A body that writes `bytes`, then truncates.
    #[must_use]
    pub const fn new(bytes: Bytes) -> Self {
        Self {
            pending: Some(bytes),
            stage: Stage::Pending,
        }
    }
}

impl Body for Truncating {
    type Data = Bytes;
    type Error = TruncatedStream;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.get_mut();

        match this.stage {
            Stage::Pending => {
                this.stage = Stage::Flushing;
                let bytes = this.pending.take().unwrap_or_default();
                Poll::Ready(Some(Ok(Frame::data(bytes))))
            }
            Stage::Flushing => {
                // Hand the runtime back control so the listener writes what it
                // has, then fail on the very next poll. Waking immediately
                // keeps this to one extra trip rather than a stall.
                this.stage = Stage::Failed;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            Stage::Failed => Poll::Ready(Some(Err(TruncatedStream))),
        }
    }

    /// Never `true`.
    ///
    /// A listener that believed the body was complete could finish the response
    /// cleanly without ever polling for the failure, which would put the lie
    /// back in.
    fn is_end_stream(&self) -> bool {
        false
    }
}
