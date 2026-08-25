//! Percent-decoding for captured path segments.

use std::borrow::Cow;

/// Why a captured segment could not be decoded.
///
/// Each variant is a `400` with `INVALID_ARGUMENT` and reason `MALFORMED_PATH`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// A `%` with fewer than two characters after it.
    Truncated,
    /// A `%` followed by something that is not two hex digits.
    BadHex,
    /// The decoded bytes are not valid UTF-8.
    NotUtf8,
}

impl DecodeError {
    /// A short description, suitable for a `FieldViolation`'s `description`.
    pub const fn description(self) -> &'static str {
        match self {
            DecodeError::Truncated => "truncated percent-escape",
            DecodeError::BadHex => "percent-escape is not two hex digits",
            DecodeError::NotUtf8 => "decodes to invalid UTF-8",
        }
    }
}

/// A decode failure, carrying the field it belongs to.
///
/// The field travels with the error so the caller can raise a
/// `BadRequest.FieldViolation` naming what the client actually sent, rather
/// than a bare "malformed path".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureError {
    /// The protojson field path, e.g. `"book.name"`.
    pub field: &'static str,
    /// What was wrong with the encoding.
    pub kind: DecodeError,
}

/// Percent-decodes one path segment, preserving `%2F`.
///
/// That exception is the rule, not a detail. `/` separates the segments of an
/// AIP-122 resource name, so decoding `%2F` would make a captured name
/// ambiguous with a genuinely longer one: `shelves/a%2Fb` and `shelves/a/b`
/// would arrive identical, and nothing downstream could tell a two-segment name
/// holding a slash from a three-segment name.
///
/// Every other escape decodes, including multi-byte UTF-8, which is
/// percent-encoded one byte at a time.
///
/// Borrows when there is nothing to decode, which is the common case: most path
/// segments are resource ids with no escapes at all.
///
/// # Errors
///
/// See [`DecodeError`].
pub fn decode_segment(segment: &str) -> Result<Cow<'_, str>, DecodeError> {
    if !segment.contains('%') {
        return Ok(Cow::Borrowed(segment));
    }

    let bytes = segment.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] != b'%' {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        if i + 2 >= bytes.len() {
            return Err(DecodeError::Truncated);
        }
        let hi = hex_val(bytes[i + 1]).ok_or(DecodeError::BadHex)?;
        let lo = hex_val(bytes[i + 2]).ok_or(DecodeError::BadHex)?;

        let byte = (hi << 4) | lo;
        if byte == b'/' {
            // Left encoded on purpose; see this function's documentation.
            out.extend_from_slice(&bytes[i..i + 3]);
        } else {
            out.push(byte);
        }
        i += 3;
    }

    String::from_utf8(out)
        .map(Cow::Owned)
        .map_err(|_| DecodeError::NotUtf8)
}

/// Decodes one hex digit, or `None` if it is not one.
fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
