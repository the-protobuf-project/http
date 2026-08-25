//! The error type every failure funnels through.

use super::Code;
use super::details::{Detail, ErrorInfo, LocalizedMessage};
use http::{HeaderMap, HeaderValue, StatusCode, header::HeaderName};
use serde_json::{Value, json};

/// A failure, ready to be rendered as an HTTP response.
#[derive(Clone, Debug)]
pub struct Error {
    /// The canonical code, which becomes the envelope's `status`.
    pub code: Code,

    /// The HTTP status.
    ///
    /// Normally `code.status_code()`, but a caller may promote it — an
    /// `If-Match` mismatch turning `FAILED_PRECONDITION` into `412`, or a
    /// routing failure keeping its `405` instead of following `UNIMPLEMENTED`
    /// to `501`.
    pub http: StatusCode,

    /// The human-readable message. Superseded by a [`LocalizedMessage`] detail
    /// when the service supplies one.
    pub message: String,

    /// The `google.rpc` details, rendered into the envelope's `details` array.
    pub details: Vec<Detail>,

    /// Headers the failure carries directly, such as `Allow` on a `405`.
    /// Headers derived from details are added by [`Error::headers`].
    pub extra_headers: HeaderMap,
}

impl Error {
    /// Builds an error with the canonical HTTP status for `code`.
    pub fn new(code: Code, message: impl Into<String>) -> Self {
        Self {
            code,
            http: code.status_code(),
            message: message.into(),
            details: Vec::new(),
            extra_headers: HeaderMap::new(),
        }
    }

    /// Overrides the HTTP status, for the narrow cases where the canonical
    /// mapping is not the most accurate answer.
    pub fn with_http(mut self, status: StatusCode) -> Self {
        self.http = status;
        self
    }

    /// Appends one detail.
    pub fn with_detail(mut self, detail: Detail) -> Self {
        self.details.push(detail);
        self
    }

    /// Attaches the [`ErrorInfo`] AIP-193 requires on every error.
    pub fn with_error_info(
        self,
        reason: impl Into<String>,
        domain: impl Into<String>,
        metadata: impl IntoIterator<Item = (String, String)>,
    ) -> Self {
        self.with_detail(Detail::ErrorInfo(ErrorInfo {
            reason: reason.into(),
            domain: domain.into(),
            metadata: metadata.into_iter().collect(),
        }))
    }

    /// Sets one response header.
    pub fn with_header(mut self, name: HeaderName, value: HeaderValue) -> Self {
        self.extra_headers.insert(name, value);
        self
    }

    /// Guarantees an [`ErrorInfo`] is present, synthesising one from the code
    /// when the service returned none.
    ///
    /// AIP-193 requires it, and a caller who cannot tell which service failed,
    /// or why, cannot act on the error.
    pub fn ensure_error_info(mut self, domain: &str) -> Self {
        if self.error_info().is_none() {
            self.details.insert(
                0,
                Detail::ErrorInfo(ErrorInfo {
                    reason: self.code.as_str().to_string(),
                    domain: domain.to_string(),
                    metadata: Default::default(),
                }),
            );
        }
        self
    }

    /// The `ErrorInfo` detail, if one is attached.
    pub(crate) fn error_info(&self) -> Option<&ErrorInfo> {
        self.details.iter().find_map(|d| match d {
            Detail::ErrorInfo(e) => Some(e),
            _ => None,
        })
    }

    /// Removes [`Detail::DebugInfo`], which can describe the shape of the
    /// service.
    ///
    /// Called unless the runtime is explicitly configured to expose it, which
    /// it should refuse to be on a non-loopback listener.
    pub fn strip_debug_info(mut self) -> Self {
        self.details.retain(|d| !matches!(d, Detail::DebugInfo(_)));
        self
    }

    /// The message to render: a [`LocalizedMessage`] when the service supplied
    /// one, otherwise [`Error::message`].
    pub(crate) fn rendered_message(&self) -> &str {
        self.details
            .iter()
            .find_map(|d| match d {
                Detail::LocalizedMessage(LocalizedMessage { message, .. })
                    if !message.is_empty() =>
                {
                    Some(message.as_str())
                }
                _ => None,
            })
            .unwrap_or(&self.message)
    }

    /// Renders the AIP-193 envelope.
    ///
    /// `code` is the **HTTP status**, not the gRPC code. That single difference
    /// is why a grpc-gateway error body reports `3` for a bad request: it
    /// serializes the raw `google.rpc.Status`, whose `code` field holds the
    /// canonical code's number, which is not an HTTP status at all.
    pub fn to_json(&self) -> Value {
        let details: Vec<Value> = self.details.iter().map(Detail::to_json).collect();

        let mut error = serde_json::Map::new();
        error.insert("code".into(), json!(self.http.as_u16()));
        error.insert("message".into(), json!(self.rendered_message()));
        error.insert("status".into(), json!(self.code.as_str()));
        if !details.is_empty() {
            error.insert("details".into(), Value::Array(details));
        }
        json!({ "error": Value::Object(error) })
    }
}
