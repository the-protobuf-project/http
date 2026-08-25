//! The `google.rpc` detail message definitions.
//!
//! Field numbers and names match `google/rpc/error_details.proto`, so these
//! decode from and encode to the same wire bytes a Go or Java service produces.

use prost::Message;
use std::collections::HashMap;

/// Identifies the reason for an error, and is the one detail AIP-193 requires
/// on every error.
#[derive(Clone, PartialEq, Message)]
pub struct ErrorInfo {
    /// A short, uppercase, service-defined token, e.g. `RESOURCE_MISSING`.
    #[prost(string, tag = "1")]
    pub reason: String,
    /// The service that produced the error, e.g. `library.example.com`.
    #[prost(string, tag = "2")]
    pub domain: String,
    /// Structured context. Rendered sorted, so output is deterministic.
    #[prost(map = "string, string", tag = "3")]
    pub metadata: HashMap<String, String>,
}

/// Field-level validation failures. This is what a `400` from the protocol section of the README/// §4.5 carries.
#[derive(Clone, PartialEq, Message)]
pub struct BadRequest {
    /// One entry per invalid field. Validation collects every problem rather
    /// than stopping at the first.
    #[prost(message, repeated, tag = "1")]
    pub field_violations: Vec<FieldViolation>,
}

/// One invalid field.
#[derive(Clone, PartialEq, Message)]
pub struct FieldViolation {
    /// The protojson path of the offending field, e.g. `book.displayName`.
    /// This is the name the client sent, which is the only name they can act
    /// on.
    #[prost(string, tag = "1")]
    pub field: String,
    /// A human-readable explanation.
    #[prost(string, tag = "2")]
    pub description: String,
    /// A machine-readable token, e.g. `REQUIRED` or `VALUE_LENGTH`.
    #[prost(string, tag = "3")]
    pub reason: String,
}

/// How long to wait before retrying. Projected to a `Retry-After` header.
#[derive(Clone, PartialEq, Message)]
pub struct RetryInfo {
    /// The minimum delay before a retry is worth attempting.
    #[prost(message, optional, tag = "1")]
    pub retry_delay: Option<prost_types::Duration>,
}

/// Which quota the caller exhausted.
#[derive(Clone, PartialEq, Message)]
pub struct QuotaFailure {
    /// One entry per exhausted quota.
    #[prost(message, repeated, tag = "1")]
    pub violations: Vec<QuotaViolation>,
}

/// One exhausted quota.
#[derive(Clone, PartialEq, Message)]
pub struct QuotaViolation {
    /// What the quota applies to, e.g. a project or an IP.
    #[prost(string, tag = "1")]
    pub subject: String,
    /// A human-readable explanation.
    #[prost(string, tag = "2")]
    pub description: String,
}

/// Which precondition was not met.
#[derive(Clone, PartialEq, Message)]
pub struct PreconditionFailure {
    /// One entry per unmet precondition.
    #[prost(message, repeated, tag = "1")]
    pub violations: Vec<PreconditionViolation>,
}

/// One unmet precondition.
#[derive(Clone, PartialEq, Message)]
pub struct PreconditionViolation {
    /// The kind of precondition, e.g. `TOS` or `OUT_OF_STOCK`.
    #[prost(string, tag = "1")]
    pub r#type: String,
    /// What the precondition applies to.
    #[prost(string, tag = "2")]
    pub subject: String,
    /// A human-readable explanation.
    #[prost(string, tag = "3")]
    pub description: String,
}

/// Which resource the error concerns.
#[derive(Clone, PartialEq, Message)]
pub struct ResourceInfo {
    /// The AIP-123 resource type, e.g. `library.example.com/Book`.
    #[prost(string, tag = "1")]
    pub resource_type: String,
    /// The AIP-122 resource name, e.g. `shelves/s1/books/b9`.
    #[prost(string, tag = "2")]
    pub resource_name: String,
    /// Who owns the resource, when that differs from the caller.
    #[prost(string, tag = "3")]
    pub owner: String,
    /// A human-readable explanation.
    #[prost(string, tag = "4")]
    pub description: String,
}

/// Links to documentation. Projected to `Link: <url>; rel="help"`.
#[derive(Clone, PartialEq, Message)]
pub struct Help {
    /// One entry per documentation link.
    #[prost(message, repeated, tag = "1")]
    pub links: Vec<HelpLink>,
}

/// One documentation link.
#[derive(Clone, PartialEq, Message)]
pub struct HelpLink {
    /// What the link explains.
    #[prost(string, tag = "1")]
    pub description: String,
    /// The link target.
    #[prost(string, tag = "2")]
    pub url: String,
}

/// A message localized for the caller, which the envelope's `message` prefers
/// when present.
#[derive(Clone, PartialEq, Message)]
pub struct LocalizedMessage {
    /// An IETF BCP 47 language tag, e.g. `fr-FR`.
    #[prost(string, tag = "1")]
    pub locale: String,
    /// The localized text.
    #[prost(string, tag = "2")]
    pub message: String,
}

/// Internal debugging state. Stripped unless explicitly exposed — it is the one
/// detail that can leak the shape of the service to a caller.
#[derive(Clone, PartialEq, Message)]
pub struct DebugInfo {
    /// Stack frames, innermost first.
    #[prost(string, repeated, tag = "1")]
    pub stack_entries: Vec<String>,
    /// Any further internal context.
    #[prost(string, tag = "2")]
    pub detail: String,
}

/// Correlation identifiers, for matching a client report to a server log.
#[derive(Clone, PartialEq, Message)]
pub struct RequestInfo {
    /// An opaque request identifier.
    #[prost(string, tag = "1")]
    pub request_id: String,
    /// Any data that helps reproduce the request.
    #[prost(string, tag = "2")]
    pub serving_data: String,
}
