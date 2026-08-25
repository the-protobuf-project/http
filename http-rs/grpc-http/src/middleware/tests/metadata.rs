//! Metadata construction and `Grpc-Timeout` parsing.

use crate::middleware::{Headers, Metadata, MetadataValue, parse_grpc_timeout};
use http::HeaderMap;
use std::time::Duration;

/// Builds a header map from pairs.
fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in pairs {
        map.insert(
            http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
            http::HeaderValue::from_str(value).unwrap(),
        );
    }
    map
}

#[test]
fn metadata_is_built_through_the_incoming_matcher() {
    let map = headers(&[
        ("grpc-metadata-trace-id", "abc123"),
        ("x-tenant-id", "acme"),
        ("connection", "keep-alive"),
    ]);
    let metadata = Metadata::from_headers(&map, &Headers::default());

    assert_eq!(metadata.get_text("trace-id"), Some("abc123"));
    assert_eq!(metadata.get_text("x-tenant-id"), Some("acme"));
    // Hop-by-hop, so it never reaches the service.
    assert_eq!(metadata.get("connection"), None);
}

#[test]
fn binary_metadata_is_base64_decoded() {
    // "hello" in standard base64.
    let map = headers(&[("grpc-metadata-payload-bin", "aGVsbG8=")]);
    let metadata = Metadata::from_headers(&map, &Headers::default());

    match metadata.get("payload-bin").and_then(<[_]>::first) {
        Some(MetadataValue::Binary(bytes)) => assert_eq!(bytes, b"hello"),
        other => panic!("expected binary metadata, got {other:?}"),
    }
}

#[test]
fn undecodable_binary_metadata_is_dropped_not_forwarded_as_text() {
    // A service reading this as binary would otherwise get silent garbage.
    let map = headers(&[("grpc-metadata-payload-bin", "not!valid!base64")]);
    let metadata = Metadata::from_headers(&map, &Headers::default());
    assert_eq!(metadata.get("payload-bin"), None);
}

#[test]
fn keys_are_lowercased_and_ordered() {
    let mut metadata = Metadata::new();
    metadata.append("Zulu", "z");
    metadata.append("Alpha", "a");

    // Sorted, so logs and test assertions are stable across runs.
    assert_eq!(metadata.keys().collect::<Vec<_>>(), vec!["alpha", "zulu"]);
}

#[test]
fn repeated_keys_accumulate() {
    let mut metadata = Metadata::new();
    metadata.append("x-tag", "one");
    metadata.append("x-tag", "two");
    assert_eq!(metadata.get("x-tag").map(<[_]>::len), Some(2));
}

#[test]
fn grpc_timeout_units_parse() {
    assert_eq!(parse_grpc_timeout("1H"), Some(Duration::from_secs(3600)));
    assert_eq!(parse_grpc_timeout("2M"), Some(Duration::from_secs(120)));
    assert_eq!(parse_grpc_timeout("30S"), Some(Duration::from_secs(30)));
    assert_eq!(parse_grpc_timeout("500m"), Some(Duration::from_millis(500)));
    assert_eq!(parse_grpc_timeout("100u"), Some(Duration::from_micros(100)));
    assert_eq!(parse_grpc_timeout("50n"), Some(Duration::from_nanos(50)));
}

#[test]
fn a_malformed_timeout_is_ignored_not_fatal() {
    // A bad timeout header should not fail an otherwise valid request; the
    // caller falls back to the configured default.
    assert_eq!(parse_grpc_timeout(""), None);
    assert_eq!(parse_grpc_timeout("30"), None);
    assert_eq!(parse_grpc_timeout("abcS"), None);
    assert_eq!(parse_grpc_timeout("30X"), None);
}
