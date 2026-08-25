//! The codec traits.
//!
//! The split into three traits is deliberate. [`Codec`] is metadata only and so
//! is object-safe, which is what lets the registry hold codecs it can negotiate
//! over without knowing any message type. [`Encode`] and [`Decode`] are generic
//! over the message and so are *not* object-safe — and that is the point: the
//! generated handler knows its concrete request and response types, so it
//! monomorphises the call and pays no dynamic dispatch on the hot path.
//!
//! Adding a codec therefore costs two impls per message type, generated, and
//! nothing at runtime.

use super::{CodecError, Framing};
use bytes::BytesMut;

/// A codec's identity: what selects it, and how it frames a stream.
///
/// Implementors are zero-sized in practice. The trait carries no encoding
/// method, so one `dyn Codec` can describe a codec for every message type at
/// once.
pub trait Codec: Send + Sync + 'static {
    /// The `?alt=` selector, e.g. `"json"`.
    ///
    /// Must be unique within a registry, and stable: it appears in client URLs.
    fn name(&self) -> &'static str;

    /// The media types this codec answers to, most canonical first.
    ///
    /// The first entry is what a response's `Content-Type` is set to; the rest
    /// are accepted aliases, e.g. `application/x-protobuf` alongside
    /// `application/protobuf`.
    fn media_types(&self) -> &'static [&'static str];

    /// How this codec delimits a server-streaming response.
    fn framing(&self) -> Framing;

    /// The `Content-Type` to send with a response encoded by this codec.
    ///
    /// Defaults to the first entry of [`Codec::media_types`], which is correct
    /// for every codec that does not need a parameter.
    fn content_type(&self) -> &'static str {
        self.media_types()
            .first()
            .copied()
            .unwrap_or("application/octet-stream")
    }
}

/// Encodes a message of type `M`.
///
/// Not object-safe by design; see the module documentation.
pub trait Encode<M>: Codec {
    /// Appends the encoded message to `out`.
    ///
    /// Encoding appends rather than returning a buffer so a streaming response
    /// can reuse one allocation across every message in the stream.
    ///
    /// # Errors
    ///
    /// Returns [`CodecError::Encode`] when the message cannot be represented in
    /// this codec's format, which is a bug rather than a caller error.
    fn encode(&self, message: &M, out: &mut BytesMut) -> Result<(), CodecError>;
}

/// Decodes a message of type `M`.
///
/// Not object-safe by design; see the module documentation.
pub trait Decode<M>: Codec {
    /// Decodes a message from a complete body.
    ///
    /// # Errors
    ///
    /// Returns [`CodecError::Malformed`] when the bytes are not well-formed,
    /// [`CodecError::Field`] when a value does not fit its field, and
    /// [`CodecError::UnknownField`] when the body names a field the message
    /// does not have.
    fn decode(&self, body: &[u8]) -> Result<M, CodecError>;
}
