//! How a stream ends, and what the transport must do about it.

use crate::error::{Code, Error};
use http::{HeaderMap, HeaderName, HeaderValue};

/// The gRPC trailers a stream ends with.
///
/// Trailers are how gRPC reports an outcome that was not known when the headers
/// went out, which is exactly the streaming case. They only reach a client that
/// advertised `TE: trailers`, which is why they are never the *only* signal —
/// see [`Termination::Truncate`].
#[derive(Debug, Clone, Default)]
pub struct TrailerSet {
    /// `grpc-status`: the canonical code as a number.
    pub status: i32,
    /// `grpc-message`: the human-readable message, percent-encoded.
    pub message: String,
    /// `grpc-status-details-bin`: base64 `google.rpc.Status`, when there are
    /// details worth carrying.
    pub details: Option<String>,
}

impl TrailerSet {
    /// Trailers for a stream that completed.
    #[must_use]
    pub fn ok() -> Self {
        Self {
            status: Code::Ok as i32,
            message: String::new(),
            details: None,
        }
    }

    /// Trailers for a stream that failed.
    #[must_use]
    pub fn from_error(err: &Error) -> Self {
        Self {
            status: err.code as i32,
            message: percent_encode(&err.message),
            details: None,
        }
    }

    /// Renders the trailers as headers.
    #[must_use]
    pub fn to_headers(&self) -> HeaderMap {
        let mut map = HeaderMap::new();
        insert(&mut map, "grpc-status", &self.status.to_string());

        if !self.message.is_empty() {
            insert(&mut map, "grpc-message", &self.message);
        }
        if let Some(details) = &self.details {
            insert(&mut map, "grpc-status-details-bin", details);
        }
        map
    }

    /// The value for the `Trailer` response header, which must be advertised in
    /// the *headers* for an intermediary to preserve the trailers at all.
    #[must_use]
    pub fn advertised(&self) -> &'static str {
        "grpc-status, grpc-message"
    }
}

/// Inserts a header, skipping a value the grammar rejects.
fn insert(map: &mut HeaderMap, name: &'static str, value: &str) {
    if let (Ok(name), Ok(value)) = (
        HeaderName::from_bytes(name.as_bytes()),
        HeaderValue::from_str(value),
    ) {
        map.insert(name, value);
    }
}

/// Percent-encodes a `grpc-message`, which must be ASCII.
///
/// A status message routinely contains a resource name or a quoted value, and
/// an un-encoded newline or non-ASCII byte in a header value is a request
/// smuggling vector rather than merely a formatting problem.
fn percent_encode(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    for byte in message.bytes() {
        if byte.is_ascii_graphic() && byte != b'%' || byte == b' ' {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// How a stream ended, and what the transport must do.
///
/// This is the value that carries README §6.2 out to the listener. A
/// transport that ignores [`Termination::Truncate`] and closes cleanly reports
/// success for a failed RPC, which is the bug the whole design exists to close.
#[derive(Debug)]
pub enum Termination {
    /// The stream completed. Write the closing frame and the trailers, then
    /// end the body normally.
    Complete {
        /// The closing bytes for the framing.
        close: Vec<u8>,
        /// `grpc-status: 0`.
        trailers: TrailerSet,
    },

    /// The stream failed before any message went out, so the status line was
    /// never committed.
    ///
    /// Nothing streaming-specific has happened yet: render this as an ordinary
    /// error response with its real status.
    Deferred {
        /// The failure, to be rendered normally.
        error: Box<Error>,
    },

    /// The stream failed after committing. The status line is spent.
    ///
    /// The transport must write `frame`, send `trailers`, and then terminate
    /// the body **abnormally** — `RST_STREAM` with `INTERNAL_ERROR` on HTTP/2
    /// and HTTP/3, or closing without the terminating zero-length chunk on
    /// HTTP/1.1.
    ///
    /// Truncation is the only signal left: it makes `curl` exit non-zero and
    /// `fetch()` reject, where a clean close would not.
    Truncate {
        /// The in-band error frame, written before truncating.
        frame: Vec<u8>,
        /// The trailers describing the failure.
        trailers: TrailerSet,
        /// The failure, for the operator's log. The client's view of it is
        /// necessarily degraded, so the server's must not be.
        error: Box<Error>,
    },
}

impl Termination {
    /// Whether the body must be terminated abnormally.
    #[must_use]
    pub const fn requires_truncation(&self) -> bool {
        matches!(self, Termination::Truncate { .. })
    }

    /// The trailers to emit, if any.
    #[must_use]
    pub const fn trailers(&self) -> Option<&TrailerSet> {
        match self {
            Termination::Complete { trailers, .. } | Termination::Truncate { trailers, .. } => {
                Some(trailers)
            }
            Termination::Deferred { .. } => None,
        }
    }
}
