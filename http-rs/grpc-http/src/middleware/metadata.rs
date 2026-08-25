//! gRPC metadata, and building it from an HTTP request.

use super::headers::{Headers, is_binary};
use http::HeaderMap;
use std::collections::BTreeMap;
use std::time::Duration;

/// One metadata value: text, or base64-decoded binary for a `-bin` key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MetadataValue {
    /// An ASCII value.
    Text(String),
    /// A binary value, already decoded from base64.
    Binary(Vec<u8>),
}

impl MetadataValue {
    /// The value as text, or `None` when it is binary.
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            MetadataValue::Text(s) => Some(s),
            MetadataValue::Binary(_) => None,
        }
    }
}

/// Metadata to send with a call.
///
/// A `BTreeMap` so iteration order is stable: metadata ends up in logs and in
/// test assertions, and an order that shifts between runs makes both worse.
/// Keys are lowercase, which is what gRPC requires.
#[derive(Clone, Default, Debug)]
pub struct Metadata {
    entries: BTreeMap<String, Vec<MetadataValue>>,
}

impl Metadata {
    /// Empty metadata.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a text value.
    pub fn append(&mut self, key: &str, value: impl Into<String>) {
        self.entries
            .entry(key.to_ascii_lowercase())
            .or_default()
            .push(MetadataValue::Text(value.into()));
    }

    /// Appends a binary value, for a `-bin` key.
    pub fn append_binary(&mut self, key: &str, value: Vec<u8>) {
        self.entries
            .entry(key.to_ascii_lowercase())
            .or_default()
            .push(MetadataValue::Binary(value));
    }

    /// The values for a key.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&[MetadataValue]> {
        self.entries
            .get(&key.to_ascii_lowercase())
            .map(Vec::as_slice)
    }

    /// The first value for a key, as text.
    #[must_use]
    pub fn get_text(&self, key: &str) -> Option<&str> {
        self.get(key)?.first()?.as_text()
    }

    /// Every key, in sorted order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    /// Whether there is nothing to send.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The number of distinct keys.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Builds metadata from request headers, per an incoming matcher.
    ///
    /// A `-bin` header is base64-decoded; one that fails to decode is dropped
    /// rather than forwarded as text, since a service reading it as binary
    /// would otherwise get silent garbage.
    #[must_use]
    pub fn from_headers(headers: &HeaderMap, matchers: &Headers) -> Self {
        let mut metadata = Metadata::new();

        for (name, value) in headers {
            let Some(key) = matchers.incoming.translate(name.as_str()) else {
                continue;
            };
            let Ok(text) = value.to_str() else {
                continue;
            };
            if is_binary(&key) {
                if let Some(bytes) = decode_base64(text) {
                    metadata.append_binary(&key, bytes);
                }
            } else {
                metadata.append(&key, text);
            }
        }
        metadata
    }
}

/// Adds metadata to a call from the request.
///
/// grpc-gateway's `WithMetadata`. Several may be registered; each sees what the
/// ones before it added, so one can build on another.
pub trait MetadataAnnotator: Send + Sync + 'static {
    /// A name, for diagnostics.
    fn name(&self) -> &'static str;

    /// Adds to `metadata`, given the request headers.
    fn annotate(&self, headers: &HeaderMap, metadata: &mut Metadata);
}

impl<F> MetadataAnnotator for (&'static str, F)
where
    F: Fn(&HeaderMap, &mut Metadata) + Send + Sync + 'static,
{
    fn name(&self) -> &'static str {
        self.0
    }

    fn annotate(&self, headers: &HeaderMap, metadata: &mut Metadata) {
        (self.1)(headers, metadata);
    }
}

/// Parses a `Grpc-Timeout` header.
///
/// The gRPC wire format is a positive integer followed by a unit: `H`, `M`,
/// `S`, `m`, `u`, `n`. Returns `None` when the header is malformed, which the
/// caller treats as "no client deadline" rather than as an error — a bad
/// timeout header should not fail an otherwise valid request.
#[must_use]
pub fn parse_grpc_timeout(raw: &str) -> Option<Duration> {
    let (digits, unit) = raw.split_at(raw.len().checked_sub(1)?);
    let amount: u64 = digits.parse().ok()?;

    let nanos = match unit {
        "H" => amount.checked_mul(3_600_000_000_000)?,
        "M" => amount.checked_mul(60_000_000_000)?,
        "S" => amount.checked_mul(1_000_000_000)?,
        "m" => amount.checked_mul(1_000_000)?,
        "u" => amount.checked_mul(1_000)?,
        "n" => amount,
        _ => return None,
    };
    Some(Duration::from_nanos(nanos))
}

/// Decodes standard or URL-safe base64, with or without padding.
///
/// gRPC metadata is standard base64, but clients send the URL-safe alphabet
/// often enough that rejecting it would be unhelpful.
fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buffer = 0u32;
    let mut bits = 0u32;

    for byte in input.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            b'\r' | b'\n' => continue,
            _ => return None,
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    Some(out)
}
