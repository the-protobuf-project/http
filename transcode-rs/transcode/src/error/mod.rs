//! The AIP-193 error model.
//!
//! Every failure this crate produces — routing, negotiation, binding,
//! validation, or an RPC's own status — becomes an [`Error`] and is
//! rendered through one place. That single funnel is the structural fix for the
//! bug that motivated the project: grpc-gateway renders unary errors, stream
//! errors, and routing errors through three different paths, and they disagree
//! about both the status and the body shape.
//!
//! The model is protocol-neutral rather than HTTP-shaped: an [`Error`] carries a
//! canonical `google.rpc` code, a set of `google.rpc` details, and the HTTP
//! status that code projects to. A frontend over another protocol reuses the
//! code and the details and projects them its own way.
//!
//! See README §5 for the normative shape.

mod code;
mod details;
// Named for the type it defines; the parent module is the public facade, the
// same arrangement route/route.rs uses.
#[allow(clippy::module_inception)]
mod error;
mod headers;
mod originated;
mod status;

#[cfg(test)]
mod tests;

pub use code::Code;
pub use details::{
    BadRequest, DebugInfo, Detail, ErrorInfo, FieldViolation, Help, HelpLink, LocalizedMessage,
    PreconditionFailure, PreconditionViolation, QuotaFailure, QuotaViolation, RequestInfo,
    ResourceInfo, RetryInfo, format_duration,
};
pub use error::Error;

/// The crate's result type.
///
/// The error is boxed because [`Error`] is large — a message, a detail
/// vector, and a header map — and it rides in the `Err` of functions on the
/// request path, where success is overwhelmingly the common case. Boxing keeps
/// every `Result` on that path pointer-sized and moves the cost onto the
/// failure, which is already doing far more work than one allocation.
pub type Result<T> = std::result::Result<T, Box<Error>>;
