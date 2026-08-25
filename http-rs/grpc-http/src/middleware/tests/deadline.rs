//! The built-in interceptors.

use super::fixtures::Fixture;
use crate::middleware::builtin::{ClientIp, Deadline, RealIp};
use crate::middleware::{Interceptor, MethodPattern};
use std::time::Duration;

#[test]
fn deadline_prefers_the_client_timeout_but_caps_it() {
    let deadline = Deadline::new(Duration::from_secs(30)).with_max(Duration::from_secs(60));

    let default = Fixture::get();
    assert_eq!(
        deadline.resolve(&default.route("GetArtist", MethodPattern::Get)),
        Duration::from_secs(30)
    );

    let asked = Fixture::get().header("grpc-timeout", "5S");
    assert_eq!(
        deadline.resolve(&asked.route("GetArtist", MethodPattern::Get)),
        Duration::from_secs(5)
    );

    // A client asking for longer than the ceiling is capped, not refused: the
    // request is otherwise perfectly valid.
    let greedy = Fixture::get().header("grpc-timeout", "3600S");
    assert_eq!(
        deadline.resolve(&greedy.route("GetArtist", MethodPattern::Get)),
        Duration::from_secs(60)
    );
}

#[test]
fn deadline_forwards_the_budget_to_the_service() {
    let fixture = Fixture::get().header("grpc-timeout", "5S");
    let mut cx = fixture.route("GetArtist", MethodPattern::Get);
    Deadline::new(Duration::from_secs(30))
        .on_route(&mut cx)
        .unwrap();

    // So the backend stops working on a call the gateway has abandoned.
    assert_eq!(cx.metadata.get_text("grpc-timeout"), Some("5000m"));
}

#[test]
fn real_ip_ignores_forwarded_headers_when_no_proxy_is_trusted() {
    // X-Forwarded-For is client-controlled. Trusting it by default is how IP
    // allowlists get bypassed.
    let fixture = Fixture::get().header("x-forwarded-for", "1.2.3.4");
    let mut cx = fixture.route("GetArtist", MethodPattern::Get);

    RealIp::direct().on_route(&mut cx).unwrap();
    let ip = cx.extensions.get::<ClientIp>().expect("client ip");
    assert_eq!(ip.0.to_string(), "203.0.113.9", "must use the peer address");
}

#[test]
fn real_ip_counts_back_past_trusted_proxies() {
    // client, then two proxies. With one trusted hop, the client is the entry
    // just left of it.
    let fixture = Fixture::get().header("x-forwarded-for", "1.2.3.4, 10.0.0.1");
    let mut cx = fixture.route("GetArtist", MethodPattern::Get);

    RealIp::trusted_hops(1).on_route(&mut cx).unwrap();
    let ip = cx.extensions.get::<ClientIp>().expect("client ip");
    assert_eq!(ip.0.to_string(), "1.2.3.4");
}

#[test]
fn real_ip_falls_back_when_the_header_is_shorter_than_the_trusted_chain() {
    // A header that did not come through our own proxies is not trusted.
    let fixture = Fixture::get().header("x-forwarded-for", "1.2.3.4");
    let mut cx = fixture.route("GetArtist", MethodPattern::Get);

    RealIp::trusted_hops(3).on_route(&mut cx).unwrap();
    let ip = cx.extensions.get::<ClientIp>().expect("client ip");
    assert_eq!(ip.0.to_string(), "203.0.113.9");
}
