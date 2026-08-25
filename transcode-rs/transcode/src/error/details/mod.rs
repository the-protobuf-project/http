//! The `google.rpc` error details AIP-193 builds on.
//!
//! These are declared here with `prost` derives rather than generated, because
//! the crate would otherwise need a build-time protobuf dependency to render an
//! error — and rendering an error is the one thing that has to keep working
//! when everything else has gone wrong. They are small, stable, and defined by
//! `google/rpc/error_details.proto`.
//!
//! Each type renders to protojson so it can sit in the envelope's `details`
//! array with its `@type` tag, exactly as a Google API would return it.

mod decode;
mod encode;
mod json;
mod types;

pub use encode::format_duration;
pub use types::{
    BadRequest, DebugInfo, ErrorInfo, FieldViolation, Help, HelpLink, LocalizedMessage,
    PreconditionFailure, PreconditionViolation, QuotaFailure, QuotaViolation, RequestInfo,
    ResourceInfo, RetryInfo,
};

/// The prefix a `google.protobuf.Any` type URL carries.
pub(crate) const TYPE_PREFIX: &str = "type.googleapis.com/";

/// One entry of the envelope's `details` array.
///
/// [`Detail::Unknown`] carries a detail this crate does not model. It is
/// preserved rather than dropped, because a service is entitled to attach its
/// own detail types and a handler silently discarding them would make them
/// useless.
#[derive(Clone, Debug, PartialEq)]
pub enum Detail {
    /// Why the error happened. Required on every AIP-193 error.
    ErrorInfo(ErrorInfo),
    /// Field-level validation failures.
    BadRequest(BadRequest),
    /// How long to wait before retrying.
    RetryInfo(RetryInfo),
    /// Which quota was exhausted.
    QuotaFailure(QuotaFailure),
    /// Which precondition was not met.
    PreconditionFailure(PreconditionFailure),
    /// Which resource the error concerns.
    ResourceInfo(ResourceInfo),
    /// Links to documentation.
    Help(Help),
    /// A message localized for the caller.
    LocalizedMessage(LocalizedMessage),
    /// Internal debugging state. Stripped unless explicitly exposed.
    DebugInfo(DebugInfo),
    /// Correlation identifiers for the request.
    RequestInfo(RequestInfo),
    /// A detail type this crate does not model, kept intact.
    Unknown {
        /// The original `@type` URL.
        type_url: String,
        /// The undecoded protobuf payload.
        value: Vec<u8>,
    },
}
