//! Codecs and content negotiation.
//!
//! A codec is an identity ([`Codec`]), a pair of message operations
//! ([`Encode`] and [`Decode`]), and a stream [`Framing`]. Path captures and
//! query parameters arrive as strings and are parsed by generated typed
//! setters, so the codec boundary is narrower than it first appears: it covers
//! the request body, the response body, and how a stream is delimited. Nothing
//! else.
//!
//! # Why three traits
//!
//! [`Codec`] carries only metadata, so it is object-safe and the registry can
//! negotiate over codecs without knowing any message type. [`Encode`] and
//! [`Decode`] are generic over the message and deliberately are not: the
//! generated handler knows its concrete request and response types, so it
//! monomorphises the call and pays no dynamic dispatch per request.
//!
//! A new codec therefore costs two generated impls per message type and
//! nothing at runtime. Adding one does not touch the router, the binder, or
//! the error model.
//!
//! # Negotiation
//!
//! See README §3 In short: the request body's codec comes from
//! `Content-Type`, and the response's from `?alt=`, then `Accept`, then the
//! request codec, then the registry default — with `415` and `406` as the
//! respective failures.

mod accept;
mod error;
mod framing;
mod media;
mod negotiate;
mod registry;
mod traits;

#[cfg(feature = "json")]
mod json;
#[cfg(feature = "proto")]
mod proto;

#[cfg(test)]
mod tests;

pub use accept::{AcceptEntry, parse_accept};
pub use error::CodecError;
pub use framing::Framing;
pub use media::MediaType;
pub use negotiate::{Negotiation, request_codec, response_codec};
pub use registry::{CodecEntry, CodecRegistry};
pub use traits::{Codec, Decode, Encode};

#[cfg(feature = "json")]
pub use json::JsonCodec;
#[cfg(feature = "proto")]
pub use proto::ProtoCodec;
