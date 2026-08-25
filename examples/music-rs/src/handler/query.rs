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

/// Reads the `?alt=` response-codec selector out of a raw query string.
///
/// Read before the query is bound because negotiation happens first: a caller
/// who asked for a codec that does not exist is owed that answer whether or not
/// the rest of their query is well formed.
pub(super) fn alt(raw: &str) -> Option<String> {
    parse_query(raw).get("alt").cloned()
}

/// The query parameters every method accepts, which are therefore never
/// reported as unknown.
///
/// These are the system parameters of README §2: they select a codec, mask a
/// response, or format it, and none of them binds to a field.
pub(super) const SYSTEM_PARAMS: &[&str] = &["alt", "fields", "prettyPrint"];

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
