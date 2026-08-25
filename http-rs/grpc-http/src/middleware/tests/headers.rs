//! Header matcher behaviour.

use crate::middleware::{Headers, default_incoming, default_outgoing, default_trailer, is_binary};

#[test]
fn grpc_metadata_prefix_is_stripped_on_the_way_in() {
    assert_eq!(
        default_incoming("Grpc-Metadata-Trace-Id").as_deref(),
        Some("trace-id")
    );
}

#[test]
fn permanent_headers_are_namespaced_not_passed_through() {
    // A service asking for metadata `host` must not silently receive the
    // transport's Host header.
    assert_eq!(
        default_incoming("Host").as_deref(),
        Some("grpcgateway-host")
    );
    assert_eq!(
        default_incoming("Accept-Language").as_deref(),
        Some("grpcgateway-accept-language")
    );
}

#[test]
fn authorization_is_namespaced_like_any_permanent_header() {
    // It reaches the service, but under a name that cannot be confused with a
    // metadata key the service itself defined.
    assert_eq!(
        default_incoming("Authorization").as_deref(),
        Some("grpcgateway-authorization")
    );
}

#[test]
fn hop_by_hop_headers_are_dropped() {
    // RFC 9110 §7.6.1: these describe *this* connection, and forwarding them
    // would describe a connection the service is not on.
    for header in [
        "Connection",
        "Keep-Alive",
        "Transfer-Encoding",
        "Upgrade",
        "TE",
        "Trailer",
        "Proxy-Authenticate",
        "Proxy-Authorization",
    ] {
        assert_eq!(default_incoming(header), None, "{header}");
    }
}

#[test]
fn custom_headers_pass_through_lowercased() {
    // gRPC metadata keys are lowercase.
    assert_eq!(
        default_incoming("X-Tenant-Id").as_deref(),
        Some("x-tenant-id")
    );
}

#[test]
fn outgoing_and_trailer_prefixes_differ() {
    assert_eq!(
        default_outgoing("request-id").as_deref(),
        Some("Grpc-Metadata-request-id")
    );
    assert_eq!(
        default_trailer("grpc-status").as_deref(),
        Some("Grpc-Trailer-grpc-status")
    );
}

#[test]
fn binary_keys_are_recognised_case_insensitively() {
    assert!(is_binary("trace-bin"));
    assert!(is_binary("Trace-Bin"));
    assert!(!is_binary("trace"));
    assert!(!is_binary("binary"));
}

#[test]
fn a_custom_matcher_replaces_the_default_wholesale() {
    // Header policy is where deployments legitimately differ, so it must be
    // replaceable rather than only extensible.
    let headers = Headers {
        incoming: crate::middleware::HeaderMatcher::new("allowlist", |key| {
            let lower = key.to_ascii_lowercase();
            (lower == "x-tenant-id").then_some(lower)
        }),
        ..Headers::default()
    };

    assert_eq!(
        headers.incoming.translate("X-Tenant-Id").as_deref(),
        Some("x-tenant-id")
    );
    assert_eq!(headers.incoming.translate("Authorization"), None);
}
