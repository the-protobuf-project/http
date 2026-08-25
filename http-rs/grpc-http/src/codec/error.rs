//! Codec failures, and how they become HTTP responses.

use crate::error::{Code, FieldViolation, GatewayError};

/// Why a codec could not encode or decode a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// The body is not well-formed in the codec's own syntax: invalid JSON,
    /// a truncated protobuf frame.
    Malformed {
        /// What was wrong, for the error message.
        reason: String,
    },

    /// The body parsed, but a field's value does not fit its type — a string
    /// where a number belongs, an unparseable timestamp.
    ///
    /// Carries the protojson path so the failure becomes a `FieldViolation`
    /// naming what the client actually sent.
    Field {
        /// The protojson path, e.g. `book.publishTime`.
        path: String,
        /// What was expected.
        reason: String,
    },

    /// The body names a field the message does not have.
    ///
    /// Rejected rather than ignored: a typo in an update call should not be a
    /// silent no-op. A codec configured with `ignore_unknown_fields` never
    /// produces this.
    UnknownField {
        /// The offending field's path as the client spelled it.
        path: String,
    },

    /// The message could not be encoded, which is a bug in the service or the
    /// generated code rather than anything the caller did.
    Encode {
        /// What failed.
        reason: String,
    },
}

impl CodecError {
    /// Converts the failure into an AIP-193 error.
    ///
    /// Decode failures are the caller's to fix and become `400`; an encode
    /// failure is the service's and becomes `500`, since the request was
    /// perfectly valid and the response is what could not be produced.
    pub fn into_gateway_error(self, domain: &str, method: &str) -> GatewayError {
        match self {
            CodecError::Malformed { reason } => GatewayError::new(
                Code::InvalidArgument,
                format!("Malformed request body: {reason}"),
            )
            .with_error_info(
                "MALFORMED_BODY",
                domain,
                [("method".into(), method.into())],
            ),
            CodecError::Field { path, reason } => GatewayError::invalid_fields(
                vec![FieldViolation {
                    field: path,
                    description: reason,
                    reason: "INVALID_VALUE".into(),
                }],
                "INVALID_ARGUMENT",
                domain,
                method,
            ),
            CodecError::UnknownField { path } => GatewayError::invalid_fields(
                vec![FieldViolation {
                    field: path.clone(),
                    description: format!("Unknown field {path:?}."),
                    reason: "UNKNOWN_FIELD".into(),
                }],
                "INVALID_ARGUMENT",
                domain,
                method,
            ),
            CodecError::Encode { reason } => {
                // The caller cannot act on this, so the detail stays internal
                // and the message stays generic.
                tracing::error!(method, reason, "failed to encode response");
                GatewayError::new(Code::Internal, "Failed to encode the response.").with_error_info(
                    "ENCODE_FAILED",
                    domain,
                    [("method".into(), method.into())],
                )
            }
        }
    }

    /// Builds a [`CodecError::Malformed`] from any displayable error.
    pub fn malformed(reason: impl std::fmt::Display) -> Self {
        CodecError::Malformed {
            reason: reason.to_string(),
        }
    }

    /// Builds a [`CodecError::Encode`] from any displayable error.
    pub fn encode(reason: impl std::fmt::Display) -> Self {
        CodecError::Encode {
            reason: reason.to_string(),
        }
    }
}

impl std::fmt::Display for CodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodecError::Malformed { reason } => write!(f, "malformed body: {reason}"),
            CodecError::Field { path, reason } => write!(f, "field {path}: {reason}"),
            CodecError::UnknownField { path } => write!(f, "unknown field {path}"),
            CodecError::Encode { reason } => write!(f, "encode failed: {reason}"),
        }
    }
}

impl std::error::Error for CodecError {}
