//! Middleware: everything that runs around a call.
//!
//! # Two planes
//!
//! The HTTP plane is ordinary [`tower::Layer`] over `http::Request` — CORS,
//! body limits, compression, TLS identity. Nothing in this module duplicates
//! it, because `tower-http` already does it and the gateway being a
//! `tower::Service` makes it free.
//!
//! This module is the **message plane**: everything that needs the resolved
//! method, the bound message, or the typed response. It cannot be a
//! `tower::Layer` without erasing the types the design depends on, so it is a
//! separate trait the generated handler invokes.
//!
//! [`Interceptor`] is object-safe and covers the phases that do not need the
//! payload — which is most of them, since authn, authz, quota, audit, and
//! tracing all key on the *method*, not its fields. [`InspectRequest`] and
//! [`InspectResponse`] are typed, opt-in, and monomorphised by codegen for the
//! minority that read payloads.
//!
//! # Relationship to grpc-gateway's `ServeMuxOption`s
//!
//! grpc-gateway's extension model is seventeen option functions, each hooking a
//! different point with a different signature, and none able to see the request
//! message — which is why it has no validation. Every one of them has a
//! counterpart here:
//!
//! | `ServeMuxOption` | Here |
//! | --- | --- |
//! | `WithMiddlewares` | [`Interceptor`], via [`Stack`] |
//! | `WithMetadata` | [`MetadataAnnotator`] |
//! | `WithIncomingHeaderMatcher` | [`HeaderMatcher`] on [`Headers::incoming`] |
//! | `WithOutgoingHeaderMatcher` | [`HeaderMatcher`] on [`Headers::outgoing`] |
//! | `WithOutgoingTrailerMatcher` | [`HeaderMatcher`] on [`Headers::trailer`] |
//! | `WithForwardResponseOption` | [`Interceptor::on_response`] |
//! | `WithForwardResponseRewriter` | [`InspectResponse`] |
//! | `WithErrorHandler` | [`ErrorRenderer`] |
//! | `WithStreamErrorHandler` | [`ErrorRenderer`] — one funnel, not three |
//! | `WithRoutingErrorHandler` | [`ErrorRenderer`] — likewise |
//! | `WithMarshalerOption` | [`crate::codec::CodecRegistry`] |
//! | `WithUnescapingMode` | [`Options::unescaping`] |
//! | `WithDisablePathLengthFallback` | [`Options::path_length_fallback`] |
//! | `WithDisableHTTPMethodOverride` | [`Options::method_override`] |
//! | `WithWriteContentLength` | [`Options::write_content_length`] |
//! | `WithDisableChunkedEncoding` | [`Options::chunked_encoding`] |
//! | `WithHealthEndpointAt` / `WithHealthzEndpoint` | [`builtin::Health`] |
//!
//! Three of those collapse into one. grpc-gateway renders unary errors, stream
//! errors, and routing errors through separate handlers, and they disagree
//! about both status and body shape; here every failure leaves through
//! [`ErrorRenderer`], so it cannot.
//!
//! [`tower::Layer`]: https://docs.rs/tower/latest/tower/trait.Layer.html

pub(crate) mod context;
mod error_renderer;
mod headers;
mod interceptor;
mod metadata;
mod options;
mod selector;
mod stack;

pub mod builtin;

#[cfg(test)]
mod tests;

pub use context::{CallCx, Outcome, ResponseParts, RouteCx};
pub use error_renderer::{DefaultErrorRenderer, ErrorRenderer};
pub use headers::{
    BINARY_SUFFIX, HeaderMatcher, Headers, METADATA_HEADER_PREFIX, METADATA_PREFIX,
    METADATA_TRAILER_PREFIX, MatcherFn, default_incoming, default_outgoing, default_trailer,
    is_binary,
};
pub use interceptor::{InspectRequest, InspectResponse, Interceptor};
pub use metadata::{Metadata, MetadataAnnotator, MetadataValue, parse_grpc_timeout};
pub use options::{Options, UnescapingMode};
pub use selector::{MethodPattern, Selector};
pub use stack::{Selected, Stack};
