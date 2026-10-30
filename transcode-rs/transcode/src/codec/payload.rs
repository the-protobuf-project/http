//! Encoding and decoding by a negotiated [`CodecEntry`], for a message type the
//! caller names.
//!
//! Negotiation picks a codec at request time, but the message type is known
//! only to the generated handler. [`Payload`] is the bound that lets one call
//! site serve every codec the build enabled: it requires whatever each enabled
//! codec needs and nothing a disabled one would.

use super::{CodecEntry, CodecError};
use bytes::BytesMut;

/// The per-codec bounds [`Payload`] is assembled from.
///
/// Public because they appear as supertraits of a public trait; nothing outside
/// this crate needs to name them. Each is blanket-implemented, so a type
/// satisfies it exactly when it satisfies what the codec requires.
pub mod bounds {
    /// What the JSON codec needs: protojson-compatible serde impls, which a
    /// prost message gets from `pbjson-build`.
    #[cfg(feature = "json")]
    pub trait JsonBound: serde::Serialize + serde::de::DeserializeOwned {}
    #[cfg(feature = "json")]
    impl<T: serde::Serialize + serde::de::DeserializeOwned> JsonBound for T {}

    /// Nothing, because the JSON codec is not compiled in.
    #[cfg(not(feature = "json"))]
    pub trait JsonBound {}
    #[cfg(not(feature = "json"))]
    impl<T> JsonBound for T {}

    /// What the binary codec needs: a prost message.
    #[cfg(feature = "proto")]
    pub trait ProtoBound: prost::Message + Default {}
    #[cfg(feature = "proto")]
    impl<T: prost::Message + Default> ProtoBound for T {}

    /// Nothing, because the binary codec is not compiled in.
    #[cfg(not(feature = "proto"))]
    pub trait ProtoBound {}
    #[cfg(not(feature = "proto"))]
    impl<T> ProtoBound for T {}
}

/// A message every enabled codec can encode and decode.
///
/// Blanket-implemented: a prost message with protojson serde impls is one, and
/// so is any serde type when only the JSON codec is enabled.
pub trait Payload: bounds::JsonBound + bounds::ProtoBound + Send + Sync + 'static {}

impl<T> Payload for T where T: bounds::JsonBound + bounds::ProtoBound + Send + Sync + 'static {}

/// Encodes a message with the codec a negotiation chose.
///
/// # Errors
///
/// [`CodecError::Encode`] when the codec fails, or when the entry names a codec
/// this build does not include — which would mean the generator and the
/// feature set disagree.
pub fn encode<M: Payload>(
    entry: &CodecEntry,
    message: &M,
    out: &mut BytesMut,
) -> Result<(), CodecError> {
    match Family::of(entry) {
        #[cfg(feature = "json")]
        Family::Json => super::Encode::encode(&super::JsonCodec::new(), message, out),
        #[cfg(feature = "proto")]
        Family::Proto => super::Encode::encode(&super::ProtoCodec::new(), message, out),
        #[allow(unreachable_patterns)]
        _ => {
            let _ = (message, out);
            Err(CodecError::encode(format!(
                "codec {:?} is not compiled into this build",
                entry.name
            )))
        }
    }
}

/// Decodes a message with the codec a negotiation chose.
///
/// # Errors
///
/// Whatever the codec reports, or [`CodecError::Malformed`] when the entry
/// names a codec this build does not include.
pub fn decode<M: Payload>(entry: &CodecEntry, body: &[u8]) -> Result<M, CodecError> {
    match Family::of(entry) {
        #[cfg(feature = "json")]
        Family::Json => super::Decode::decode(&super::JsonCodec::new(), body),
        #[cfg(feature = "proto")]
        Family::Proto => super::Decode::decode(&super::ProtoCodec::new(), body),
        #[allow(unreachable_patterns)]
        _ => {
            let _ = body;
            Err(CodecError::malformed(format!(
                "codec {:?} is not compiled into this build",
                entry.name
            )))
        }
    }
}

/// How a codec entry's messages are serialized, as distinct from how they are
/// framed.
///
/// An SSE or NDJSON entry frames a stream differently from the JSON one, but
/// each message inside it is the same protojson. Deciding by the serialization
/// rather than by the entry's name is what lets a registry add a framing
/// without this function having to learn about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family {
    Json,
    Proto,
}

impl Family {
    fn of(entry: &CodecEntry) -> Self {
        if entry.name == "proto" || entry.framing == super::Framing::LengthPrefixed {
            Family::Proto
        } else {
            Family::Json
        }
    }
}
