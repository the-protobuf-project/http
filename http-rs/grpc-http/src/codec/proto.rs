//! The binary protobuf codec.

use super::{Codec, CodecError, Decode, Encode, Framing};
use bytes::BytesMut;
use prost::Message;

/// Encodes and decodes the protobuf wire format.
///
/// This is the same encoding gRPC carries, without the gRPC framing. It exists
/// for clients that want the wire format's compactness and exactness over an
/// ordinary HTTP request — and, more usefully, as the second codec that proves
/// the abstraction is real. A one-codec seam is a guess.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProtoCodec;

impl ProtoCodec {
    /// Builds the codec.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Codec for ProtoCodec {
    fn name(&self) -> &'static str {
        "proto"
    }

    fn media_types(&self) -> &'static [&'static str] {
        // application/x-protobuf is the more widely sent of the two, so it is
        // canonical here and appears as the response Content-Type.
        &["application/x-protobuf", "application/protobuf"]
    }

    fn framing(&self) -> Framing {
        // Line-delimiting bytes that may themselves contain a newline does not
        // work, so a binary codec has to carry explicit lengths.
        Framing::LengthPrefixed
    }
}

impl<M: Message> Encode<M> for ProtoCodec {
    fn encode(&self, message: &M, out: &mut BytesMut) -> Result<(), CodecError> {
        out.reserve(message.encoded_len());
        message.encode(out).map_err(CodecError::encode)
    }
}

impl<M: Message + Default> Decode<M> for ProtoCodec {
    fn decode(&self, body: &[u8]) -> Result<M, CodecError> {
        // An empty body decodes to a message with every field at its default,
        // which is exactly what protobuf says an empty encoding means.
        M::decode(body).map_err(|err| CodecError::Malformed {
            reason: err.to_string(),
        })
    }
}
