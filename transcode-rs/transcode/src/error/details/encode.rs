//! protojson value encodings shared by the detail renderers.

/// Formats a `google.protobuf.Duration` the way protojson does: decimal seconds
/// with an `s` suffix, and fractional digits trimmed to 0, 3, 6, or 9.
///
/// The proto permits a mixed-sign representation — positive seconds with
/// negative nanos — which is normalized here before rendering, so `1s` minus
/// `500ms` renders as `"0.500s"` rather than something nonsensical.
pub fn format_duration(d: &prost_types::Duration) -> String {
    let (mut secs, mut nanos) = (d.seconds, d.nanos);
    if secs > 0 && nanos < 0 {
        secs -= 1;
        nanos += 1_000_000_000;
    } else if secs < 0 && nanos > 0 {
        secs += 1;
        nanos -= 1_000_000_000;
    }

    let sign = if secs < 0 || nanos < 0 { "-" } else { "" };
    let (secs, nanos) = (secs.unsigned_abs(), nanos.unsigned_abs());

    if nanos == 0 {
        format!("{sign}{secs}s")
    } else if nanos % 1_000_000 == 0 {
        format!("{sign}{secs}.{:03}s", nanos / 1_000_000)
    } else if nanos % 1_000 == 0 {
        format!("{sign}{secs}.{:06}s", nanos / 1_000)
    } else {
        format!("{sign}{secs}.{nanos:09}s")
    }
}

/// Encodes bytes as standard, padded base64, which is what protojson uses for
/// a `bytes` field.
///
/// Written out rather than pulled from a crate because the error path must have
/// as few dependencies as possible: this runs when something has already gone
/// wrong.
pub(super) fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = u32::from_be_bytes([0, b[0], b[1], b[2]]);

        out.push(ALPHABET[(n >> 18 & 63) as usize] as char);
        out.push(ALPHABET[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}
