//! The built-in interceptors.

use super::fixtures::{DOMAIN, Fixture};
use crate::error::Code;
use crate::middleware::builtin::{Cors, Health, Limiter, RateLimit, ServingStatus};
use crate::middleware::{Interceptor, MethodPattern, ResponseParts};
use std::time::Duration;

/// Refuses everything, with a fixed delay.
struct AlwaysOver;

impl Limiter for AlwaysOver {
    fn allow(&self, _: &str, _: &str) -> Result<(), Duration> {
        Err(Duration::from_secs(2))
    }
}

#[test]
fn rate_limit_returns_429_with_quota_and_retry_details() {
    let fixture = Fixture::get();
    let mut cx = fixture.route("GetArtist", MethodPattern::Get);
    let err = RateLimit::new(AlwaysOver, DOMAIN)
        .on_route(&mut cx)
        .unwrap_err();

    assert_eq!(err.http.as_u16(), 429);
    assert_eq!(err.code, Code::ResourceExhausted);
    // RetryInfo projects to Retry-After, so a client knows when to return.
    assert_eq!(err.headers().get(http::header::RETRY_AFTER).unwrap(), "2");

    let rendered = err.to_json().to_string();
    assert!(rendered.contains("QuotaFailure"), "{rendered}");
}

#[test]
fn health_reports_serving_status_as_an_http_status() {
    let health = Health::healthz();
    assert!(health.handles("/healthz"));
    assert!(!health.handles("/v1/artists"));

    let (parts, body) = health.respond(None);
    assert_eq!(parts.status, http::StatusCode::OK);
    // Health checks are polled constantly and must never be cached.
    assert_eq!(
        parts.headers.get(http::header::CACHE_CONTROL).unwrap(),
        "no-store"
    );
    assert_eq!(String::from_utf8(body).unwrap(), r#"{"status":"SERVING"}"#);

    let down = Health::at("/healthz", |_| ServingStatus::NotServing);
    assert_eq!(down.respond(None).0.status.as_u16(), 503);

    // Unknown is 404, not 503: 503 would imply the service exists.
    let unknown = Health::at("/healthz", |_| ServingStatus::ServiceUnknown);
    assert_eq!(unknown.respond(None).0.status.as_u16(), 404);
}

#[test]
fn health_reads_the_service_query_parameter() {
    let health = Health::at("/healthz", |service| match service {
        Some("music.v1.ArtistService") => ServingStatus::Serving,
        Some(_) => ServingStatus::ServiceUnknown,
        None => ServingStatus::Serving,
    });
    assert_eq!(
        health
            .respond(Some("service=music.v1.ArtistService"))
            .0
            .status
            .as_u16(),
        200
    );
    assert_eq!(health.respond(Some("service=other")).0.status.as_u16(), 404);
}

#[test]
fn cors_echoes_only_allowlisted_origins() {
    let cors = Cors::allow(["https://app.example.com"]);
    let fixture = Fixture::get().header("origin", "https://app.example.com");
    let mut call = fixture.call("GetArtist", MethodPattern::Get);
    let mut parts = ResponseParts::ok();

    cors.on_response(&mut call, &mut parts).unwrap();
    assert_eq!(
        parts
            .headers
            .get(http::header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .unwrap(),
        "https://app.example.com"
    );
    // An allowlisted response varies by origin, so a shared cache must not
    // serve one origin's response to another.
    assert_eq!(parts.headers.get(http::header::VARY).unwrap(), "Origin");

    let other = Fixture::get().header("origin", "https://evil.example.com");
    let mut call = other.call("GetArtist", MethodPattern::Get);
    let mut parts = ResponseParts::ok();
    cors.on_response(&mut call, &mut parts).unwrap();
    assert!(
        !parts
            .headers
            .contains_key(http::header::ACCESS_CONTROL_ALLOW_ORIGIN)
    );
}

#[test]
#[should_panic(expected = "credentialed CORS requires an explicit origin allowlist")]
fn cors_refuses_wildcard_with_credentials() {
    // The Fetch standard rejects `*` with credentials, and a browser silently
    // refuses the response. Failing at construction is better.
    let _ = Cors::permissive().with_credentials();
}
