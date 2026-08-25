//! A registry mirroring the four codecs README §3 defines.

use crate::codec::{CodecEntry, CodecRegistry, Framing};

/// The registry index of each codec, matching declaration order.
pub const JSON: usize = 0;
/// See [`JSON`].
pub const PROTO: usize = 1;
/// See [`JSON`].
pub const NDJSON: usize = 2;
/// See [`JSON`].
pub const SSE: usize = 3;

/// The codec table, JSON first so it is the default.
pub static CODECS: &[CodecEntry] = &[
    CodecEntry {
        name: "json",
        media_types: &["application/json"],
        framing: Framing::JsonArray,
        index: JSON,
    },
    CodecEntry {
        name: "proto",
        media_types: &["application/x-protobuf", "application/protobuf"],
        framing: Framing::LengthPrefixed,
        index: PROTO,
    },
    CodecEntry {
        name: "ndjson",
        media_types: &["application/x-ndjson"],
        framing: Framing::LineDelimited,
        index: NDJSON,
    },
    CodecEntry {
        name: "sse",
        media_types: &["text/event-stream"],
        framing: Framing::Sse,
        index: SSE,
    },
];

/// The registry under test.
pub fn registry() -> CodecRegistry {
    CodecRegistry::new(CODECS)
}

/// The API domain used in error assertions.
pub const DOMAIN: &str = "library.example.com";
