//! A rendered response, before it reaches the wire.

use crate::error::Error;
use bytes::Bytes;
use http::{HeaderMap, HeaderValue, StatusCode, header};

/// The envelope sent when the error itself cannot be rendered.
///
/// Hand-written rather than an empty 500: the only way rendering fails is a
/// detail carrying an unrenderable value, and a caller who receives an empty
/// body learns nothing at all.
pub(crate) const FALLBACK_ENVELOPE: &[u8] =
    br#"{"error":{"code":500,"message":"Internal error.","status":"INTERNAL"}}"#;

/// A rendered HTTP response.
///
/// A dispatcher returns one fully formed rather than writing to the
/// connection, which is what makes the no-false-2xx rule structural for unary
/// calls: the body is already encoded when the status line is written, so an
/// encoding failure cannot arrive after a `200` has gone out.
#[derive(Debug, Clone)]
pub struct Reply {
    /// The status line.
    pub status: StatusCode,
    /// The response headers.
    pub headers: HeaderMap,
    /// The encoded body.
    pub body: Bytes,
}

impl Reply {
    /// A reply with a status and a body, and no headers yet.
    #[must_use]
    pub fn new(status: StatusCode, body: impl Into<Bytes>) -> Self {
        Self {
            status,
            headers: HeaderMap::new(),
            body: body.into(),
        }
    }

    /// A `204 No Content`.
    #[must_use]
    pub fn no_content() -> Self {
        Self::new(StatusCode::NO_CONTENT, Bytes::new())
    }

    /// Sets one header, ignoring a value the header grammar rejects.
    #[must_use]
    pub fn with_header(mut self, name: header::HeaderName, value: &str) -> Self {
        if let Ok(value) = HeaderValue::from_str(value) {
            self.headers.insert(name, value);
        }
        self
    }

    /// Whether the status forbids a body.
    ///
    /// A body on a `204` or `304` is a protocol violation, so one built anyway
    /// is dropped here, where it can be seen, rather than by the server.
    pub(crate) fn forbids_body(&self) -> bool {
        self.status == StatusCode::NO_CONTENT
            || self.status == StatusCode::NOT_MODIFIED
            || self.status.is_informational()
    }

    /// Renders an error as an AIP-193 response.
    pub(crate) fn error(err: &Error) -> Self {
        let (status, body) = match serde_json::to_vec(&err.to_json()) {
            Ok(body) => (err.http, Bytes::from(body)),
            Err(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Bytes::from_static(FALLBACK_ENVELOPE),
            ),
        };
        let mut headers = err.headers();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        Self {
            status,
            headers,
            body,
        }
    }
}

/// Renders an error envelope for an in-band stream frame.
///
/// JSON regardless of the negotiated codec: the envelope is the one message in
/// the protocol with no generated type behind it, and a client that cannot
/// parse its own error frame learns nothing from it.
pub(crate) fn encode_error(err: &Error) -> Vec<u8> {
    serde_json::to_vec(&err.to_json()).unwrap_or_else(|_| FALLBACK_ENVELOPE.to_vec())
}
