//! Failures the handler originates itself, before or instead of an RPC.
//!
//! Each carries a reason token from the README §5.4 set, so a caller can
//! tell a handler rejection from a service one without parsing prose.

use super::details::{BadRequest, Detail, FieldViolation};
use super::{Code, Error};
use http::{HeaderValue, StatusCode, header};

impl Error {
    /// No route matched the request path.
    pub fn route_not_found(path: &str, domain: &str) -> Self {
        Error::new(Code::NotFound, format!("No route matches {path:?}.")).with_error_info(
            "ROUTE_NOT_FOUND",
            domain,
            [("path".into(), path.into())],
        )
    }

    /// The path matched but the HTTP method did not.
    ///
    /// This stays a `405` and carries `Allow`. grpc-gateway routes it through
    /// `codes.Unimplemented`, which its own status table maps back out as
    /// `501`, losing both the status and the header a client needs to recover.
    pub fn method_not_allowed(method: &str, allow: &[&str], domain: &str) -> Self {
        let allow_value = allow.join(", ");
        let mut err = Error::new(
            Code::Unimplemented,
            format!("Method {method} is not allowed on this path."),
        )
        .with_http(StatusCode::METHOD_NOT_ALLOWED)
        .with_error_info(
            "METHOD_NOT_ALLOWED",
            domain,
            [("allow".into(), allow_value.clone())],
        );

        if let Ok(v) = HeaderValue::from_str(&allow_value) {
            err.extra_headers.insert(header::ALLOW, v);
        }
        err
    }

    /// The request's `Content-Type` names no registered codec.
    pub fn unsupported_media_type(got: &str, supported: &[&str], domain: &str) -> Self {
        Error::new(
            Code::InvalidArgument,
            format!("Content-Type {got:?} is not supported."),
        )
        .with_http(StatusCode::UNSUPPORTED_MEDIA_TYPE)
        .with_error_info(
            "UNSUPPORTED_MEDIA_TYPE",
            domain,
            [("supported".into(), supported.join(", "))],
        )
    }

    /// Nothing in the request's `Accept` names a registered codec.
    ///
    /// This is a rejection rather than a fallback: answering in a media type
    /// the client excluded is worse than telling them there is no overlap.
    pub fn not_acceptable(accept: &str, supported: &[&str], domain: &str) -> Self {
        Error::new(
            Code::InvalidArgument,
            format!("No supported media type satisfies Accept: {accept}."),
        )
        .with_http(StatusCode::NOT_ACCEPTABLE)
        .with_error_info(
            "NOT_ACCEPTABLE",
            domain,
            [("supported".into(), supported.join(", "))],
        )
    }

    /// One or more request fields are invalid.
    ///
    /// Every violation is reported at once, so a caller fixes everything in one
    /// round trip rather than discovering problems one at a time.
    pub fn invalid_fields(
        violations: Vec<FieldViolation>,
        reason: &str,
        domain: &str,
        method: &str,
    ) -> Self {
        let message = match violations.len() {
            1 => format!(
                "Request contains an invalid field: {}.",
                violations[0].field
            ),
            n => format!("Request contains {n} invalid fields."),
        };

        Error::new(Code::InvalidArgument, message)
            .with_error_info(reason, domain, [("method".into(), method.into())])
            .with_detail(Detail::BadRequest(BadRequest {
                field_violations: violations,
            }))
    }

    /// A caught panic.
    ///
    /// Rendered as an ordinary `500`: the payload never reaches the client, and
    /// the connection is not dropped, because an unwind in one handler is not a
    /// reason to fail every in-flight request on the same connection.
    pub fn panicked(domain: &str, method: &str) -> Self {
        Error::new(Code::Internal, "Internal error.").with_error_info(
            "GATEWAY_PANIC",
            domain,
            [("method".into(), method.into())],
        )
    }

    /// `413 Payload Too Large`: the body exceeded the configured limit.
    ///
    /// Decided by reading one byte past the limit rather than trusting
    /// `Content-Length`, which a client controls and a chunked request omits.
    #[must_use]
    pub fn payload_too_large(limit: usize, domain: &str) -> Self {
        Error::new(Code::InvalidArgument, "Request body is too large.")
            .with_http(StatusCode::PAYLOAD_TOO_LARGE)
            .with_error_info(
                "PAYLOAD_TOO_LARGE",
                domain,
                [("limit".into(), limit.to_string())],
            )
    }

    /// `400`: a path capture matched a route but could not be decoded.
    ///
    /// A 400 naming the field rather than a 404: the path matched, and the value
    /// is what is wrong. "Not found" would send a caller looking for a resource
    /// when the fix is their encoding.
    #[must_use]
    pub fn malformed_path(field: &str, description: &str, domain: &str, method: &str) -> Self {
        Error::invalid_fields(
            vec![FieldViolation {
                field: field.to_string(),
                description: description.to_string(),
                reason: "MALFORMED_PATH".into(),
            }],
            "MALFORMED_PATH",
            domain,
            method,
        )
    }

    /// `400`: the body could not be read or decoded.
    #[must_use]
    pub fn malformed_body(description: &str, domain: &str, method: &str) -> Self {
        Error::new(Code::InvalidArgument, "Request body could not be decoded.").with_error_info(
            "MALFORMED_BODY",
            domain,
            [
                ("method".into(), method.into()),
                ("detail".into(), description.into()),
            ],
        )
    }

    /// `400`: a body was sent to a method whose binding accepts none.
    #[must_use]
    pub fn unexpected_body(domain: &str, method: &str) -> Self {
        Error::new(
            Code::InvalidArgument,
            "This method does not accept a request body.",
        )
        .with_error_info("MALFORMED_BODY", domain, [("method".into(), method.into())])
    }

    /// `400`: query parameters that bind to no field.
    ///
    /// Rejecting is the point: grpc-gateway discards unknown parameters, which
    /// turns a typo in an update call into a silent no-op.
    #[must_use]
    pub fn unknown_query_parameter(names: &[&str], domain: &str, method: &str) -> Self {
        Error::invalid_fields(
            names
                .iter()
                .map(|name| FieldViolation {
                    field: (*name).to_string(),
                    description: format!("Unknown query parameter {name:?}."),
                    reason: "UNKNOWN_QUERY_PARAMETER".into(),
                })
                .collect(),
            "UNKNOWN_QUERY_PARAMETER",
            domain,
            method,
        )
    }

    /// `400`: a `$`-prefixed query parameter that is not a system parameter.
    ///
    /// The prefix is reserved so a future system parameter cannot collide with a
    /// field name; accepting an unknown one would let that future parameter
    /// silently change what an existing request means.
    #[must_use]
    pub fn reserved_query_parameter(name: &str, domain: &str, method: &str) -> Self {
        Error::new(
            Code::InvalidArgument,
            format!("Query parameter {name:?} is reserved."),
        )
        .with_error_info(
            "UNKNOWN_QUERY_PARAMETER",
            domain,
            [
                ("method".into(), method.into()),
                ("parameter".into(), name.into()),
            ],
        )
    }

    /// `500`: the route table and the handler disagree about a capture.
    ///
    /// A generator bug rather than a caller error, so it is a 500, not a 400.
    #[must_use]
    pub fn binding_mismatch(field: &str, domain: &str, method: &str) -> Self {
        Error::new(Code::Internal, format!("Route did not bind {field:?}."))
            .with_http(StatusCode::INTERNAL_SERVER_ERROR)
            .with_error_info(
                "BINDING_MISMATCH",
                domain,
                [("method".into(), method.into())],
            )
    }
}
