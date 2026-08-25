//! Query-string parsing and the malformed-path error.

use crate::generated::DOMAIN;
use std::collections::HashMap;
use transcode::error::Error;

/// Builds the `400` for a path segment whose percent-encoding is broken.
pub(super) fn malformed_path(err: &transcode::route::CaptureError, path: &str) -> Error {
    Error::invalid_fields(
        vec![transcode::error::FieldViolation {
            field: err.field.to_string(),
            description: err.kind.description().to_string(),
            reason: "MALFORMED_PATH".into(),
        }],
        "MALFORMED_PATH",
        DOMAIN,
        path,
    )
}

/// Parses a query string into decoded key/value pairs.
///
/// Repeated keys keep the last value, which is enough for this fixture; the
/// generator emits a repeated-aware binder for fields that are actually
/// repeated.
pub(super) fn parse_query(raw: &str) -> HashMap<String, String> {
    raw.split('&')
        .filter(|pair| !pair.is_empty())
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            Some((decode(key), decode(value)))
        })
        .collect()
}

/// Percent-decodes a query component, where `+` means a space.
fn decode(raw: &str) -> String {
    let replaced = raw.replace('+', " ");
    percent_decode(&replaced)
}

/// Percent-decodes fully. Unlike a path segment, a query value has no `%2F`
/// exception: `/` carries no structural meaning here.
fn percent_decode(raw: &str) -> String {
    percent_encoding::percent_decode_str(raw)
        .decode_utf8_lossy()
        .into_owned()
}
