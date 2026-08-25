//! The protojson codec.

use super::{Codec, CodecError, Decode, Encode, Framing};
use bytes::{BufMut, BytesMut};
use serde::{Serialize, de::DeserializeOwned};

/// Encodes and decodes protojson.
///
/// The protojson *mapping* — lowerCamelCase field names, enums as strings,
/// 64-bit integers as strings, `Timestamp` as RFC 3339 — is a property of the
/// generated `Serialize` and `Deserialize` implementations, not of this type.
/// That separation is what keeps a future codec over the same messages from
/// inheriting semantics that make no sense for it.
///
/// See README §4.1 for the mapping this codec's generated impls follow.
#[derive(Debug, Clone, Copy, Default)]
pub struct JsonCodec {
    /// Indent the output, as `?prettyPrint=true` requests.
    pub pretty: bool,
}

impl JsonCodec {
    /// A compact codec, which is the default.
    #[must_use]
    pub const fn new() -> Self {
        Self { pretty: false }
    }

    /// An indented codec, for `?prettyPrint=true`.
    #[must_use]
    pub const fn pretty() -> Self {
        Self { pretty: true }
    }
}

impl Codec for JsonCodec {
    fn name(&self) -> &'static str {
        "json"
    }

    fn media_types(&self) -> &'static [&'static str] {
        &["application/json"]
    }

    fn framing(&self) -> Framing {
        Framing::JsonArray
    }
}

impl<M: Serialize> Encode<M> for JsonCodec {
    fn encode(&self, message: &M, out: &mut BytesMut) -> Result<(), CodecError> {
        let writer = out.writer();
        if self.pretty {
            serde_json::to_writer_pretty(writer, message).map_err(CodecError::encode)
        } else {
            serde_json::to_writer(writer, message).map_err(CodecError::encode)
        }
    }
}

impl<M: DeserializeOwned> Decode<M> for JsonCodec {
    fn decode(&self, body: &[u8]) -> Result<M, CodecError> {
        // An empty body is a valid message with every field at its default,
        // which is what `POST /v1/books` with no body means.
        if body.is_empty() {
            return serde_json::from_slice(b"{}").map_err(into_codec_error);
        }
        serde_json::from_slice(body).map_err(into_codec_error)
    }
}

/// Classifies a `serde_json` failure into the right `CodecError`.
///
/// The distinction decides what the client is told: a syntax error means the
/// body is not JSON at all, while a data error means it is JSON that does not
/// fit the message — and the second can name the field responsible.
fn into_codec_error(err: serde_json::Error) -> CodecError {
    use serde_json::error::Category;

    match err.classify() {
        Category::Syntax | Category::Eof => CodecError::Malformed {
            reason: format!(
                "invalid JSON at line {}, column {}",
                err.line(),
                err.column()
            ),
        },
        Category::Data => match unknown_field_path(&err) {
            Some(path) => CodecError::UnknownField { path },
            None => CodecError::Field {
                path: String::new(),
                reason: err.to_string(),
            },
        },
        Category::Io => CodecError::Malformed {
            reason: err.to_string(),
        },
    }
}

/// Extracts the field name from serde's "unknown field" message.
///
/// serde does not expose the name structurally, and the alternative — treating
/// every data error alike — would lose the distinction between "you sent a
/// field that does not exist" and "you sent the wrong type", which are
/// different problems for the caller to fix.
fn unknown_field_path(err: &serde_json::Error) -> Option<String> {
    let text = err.to_string();
    let rest = text.strip_prefix("unknown field `")?;
    let (name, _) = rest.split_once('`')?;
    Some(name.to_string())
}
