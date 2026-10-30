//! Routing, binding, negotiation, the middleware phases, and the error funnel.

use super::fixtures::{Catalog, DOMAIN, builder, collect, gateway, get, post};
use crate::error::{Code, Error, Result};
use crate::middleware::builtin::Deadline;
use crate::middleware::{CallCx, Interceptor, Outcome, ResponseParts, RouteCx, Selector};
use http::StatusCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

#[tokio::test]
async fn a_unary_call_is_served_with_its_length() {
    let reply = collect(gateway().handle(get("/v1/artists/miles")).await).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.headers["content-type"], "application/json");
    assert_eq!(
        reply.headers["content-length"],
        reply.body.len().to_string().as_str()
    );
    assert_eq!(reply.json()["displayName"], "Miles Davis");
}

#[tokio::test]
async fn an_unmatched_path_is_a_404_envelope() {
    let reply = collect(gateway().handle(get("/v1/nothing")).await).await;

    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    assert_eq!(reply.json()["error"]["status"], "NOT_FOUND");
    assert_eq!(reply.reason(), "ROUTE_NOT_FOUND");
}

#[tokio::test]
async fn a_wrong_method_is_a_405_with_allow() {
    let request = http::Request::put("/v1/artists/miles")
        .body(http_body_util::Full::new(bytes::Bytes::new()))
        .unwrap();
    let reply = collect(gateway().handle(request).await).await;

    assert_eq!(reply.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(reply.headers["allow"], "GET");
    assert_eq!(reply.reason(), "METHOD_NOT_ALLOWED");
}

#[tokio::test]
async fn a_service_status_keeps_its_code() {
    let reply = collect(gateway().handle(get("/v1/artists/nobody")).await).await;

    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    assert_eq!(
        reply.json()["error"]["message"],
        "artists/nobody does not exist."
    );
}

#[tokio::test]
async fn an_unknown_query_parameter_is_rejected() {
    let reply = collect(gateway().handle(get("/v1/artists/miles?vew=full")).await).await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.reason(), "UNKNOWN_QUERY_PARAMETER");
}

#[tokio::test]
async fn a_reserved_query_parameter_is_rejected() {
    let reply = collect(gateway().handle(get("/v1/artists/miles?$bogus=1")).await).await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        reply.json()["error"]["message"],
        "Query parameter \"$bogus\" is reserved."
    );
}

#[tokio::test]
async fn an_unsupported_media_type_is_a_415() {
    let reply = collect(
        gateway()
            .handle(post("/v1/artists", "application/xml", "<a/>"))
            .await,
    )
    .await;

    assert_eq!(reply.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(reply.reason(), "UNSUPPORTED_MEDIA_TYPE");
}

#[tokio::test]
async fn an_unsatisfiable_accept_is_a_406() {
    let request = http::Request::get("/v1/artists/miles")
        .header("accept", "application/xml")
        .body(http_body_util::Full::new(bytes::Bytes::new()))
        .unwrap();
    let reply = collect(gateway().handle(request).await).await;

    assert_eq!(reply.status, StatusCode::NOT_ACCEPTABLE);
}

#[tokio::test]
async fn a_create_answers_201_with_its_location() {
    let reply = collect(
        gateway()
            .handle(post(
                "/v1/artists",
                "application/json",
                r#"{"name":"artists/bill","displayName":"Bill Evans"}"#,
            ))
            .await,
    )
    .await;

    assert_eq!(reply.status, StatusCode::CREATED);
    assert_eq!(reply.headers["location"], "/v1/artists/bill");
}

#[tokio::test]
async fn a_malformed_body_is_a_400() {
    let reply = collect(
        gateway()
            .handle(post("/v1/artists", "application/json", "{not json"))
            .await,
    )
    .await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_body_over_the_limit_is_a_413() {
    let gateway = builder(Catalog).max_body_bytes(8).build();
    let reply = collect(
        gateway
            .handle(post(
                "/v1/artists",
                "application/json",
                r#"{"name":"artists/bill"}"#,
            ))
            .await,
    )
    .await;

    assert_eq!(reply.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(reply.reason(), "PAYLOAD_TOO_LARGE");
}

#[tokio::test]
async fn a_panicking_dispatcher_is_an_ordinary_500() {
    let reply = collect(gateway().handle(get("/v1/artists/miles?panic")).await).await;

    assert_eq!(reply.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(reply.reason(), "GATEWAY_PANIC");
}

/// Rejects mutating calls in the route phase, and counts completions.
struct Guard {
    completed: Arc<AtomicUsize>,
}

impl Interceptor for Guard {
    fn name(&self) -> &'static str {
        "guard"
    }

    fn on_route(&self, cx: &mut RouteCx<'_>) -> Result<()> {
        if cx.headers.contains_key("authorization") {
            return Ok(());
        }
        Err(Box::new(
            Error::new(Code::Unauthenticated, "Sign in first.").with_error_info(
                "UNAUTHENTICATED",
                DOMAIN,
                [],
            ),
        ))
    }

    fn on_complete(&self, _cx: &CallCx<'_>, outcome: &Outcome<'_>) {
        assert_eq!(outcome.status(), StatusCode::UNAUTHORIZED);
        self.completed.fetch_add(1, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn a_route_phase_rejection_skips_the_call_but_completes() {
    let completed = Arc::new(AtomicUsize::new(0));
    let gateway = builder(Catalog)
        .layer_on(
            Guard {
                completed: Arc::clone(&completed),
            },
            Selector::Mutating,
        )
        .build();

    let rejected = collect(
        gateway
            .handle(post("/v1/artists", "application/json", "{}"))
            .await,
    )
    .await;
    assert_eq!(rejected.status, StatusCode::UNAUTHORIZED);
    assert_eq!(completed.load(Ordering::SeqCst), 1);

    // A read is not selected, so it is served.
    let read = collect(gateway.handle(get("/v1/artists/miles")).await).await;
    assert_eq!(read.status, StatusCode::OK);
}

/// Promotes every success to `202` and stamps a header.
struct Stamp;

impl Interceptor for Stamp {
    fn name(&self) -> &'static str {
        "stamp"
    }

    fn on_response(&self, _cx: &mut CallCx<'_>, parts: &mut ResponseParts) -> Result<()> {
        assert_eq!(parts.status, StatusCode::OK, "sees the handler's status");
        parts.status = StatusCode::ACCEPTED;
        parts.set(http::header::HeaderName::from_static("x-stamped"), "yes");
        Ok(())
    }
}

#[tokio::test]
async fn the_response_phase_can_change_the_status() {
    let gateway = builder(Catalog).layer(Stamp).build();
    let reply = collect(gateway.handle(get("/v1/artists/miles")).await).await;

    assert_eq!(reply.status, StatusCode::ACCEPTED);
    assert_eq!(reply.headers["x-stamped"], "yes");
}

/// Captures what a service would be called with.
#[derive(Debug, Default)]
struct Echo;

impl crate::gateway::Dispatcher for Echo {
    async fn dispatch(&self, call: &crate::gateway::Call<'_>) -> Result<crate::gateway::Reply> {
        let request = call.request(());
        let metadata = request.metadata();
        let timeout = metadata
            .get("grpc-timeout")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let user = metadata
            .get("x-user")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        call.ok(&super::fixtures::Artist {
            name: timeout,
            display_name: user,
        })
    }
}

#[tokio::test]
async fn a_service_request_carries_metadata_and_the_deadline() {
    let gateway = builder(Echo)
        .layer(Deadline::new(Duration::from_secs(5), DOMAIN))
        .build();
    let request = http::Request::get("/v1/artists/miles")
        .header("grpc-metadata-x-user", "ada")
        .body(http_body_util::Full::new(bytes::Bytes::new()))
        .unwrap();
    let reply = collect(gateway.handle(request).await).await;

    assert_eq!(reply.status, StatusCode::OK);
    let json = reply.json();
    assert_eq!(json["displayName"], "ada");
    assert!(
        !json["name"].as_str().unwrap().is_empty(),
        "the deadline interceptor's timeout reached the service: {json}"
    );
}

#[tokio::test]
async fn a_health_endpoint_answers_ahead_of_routing() {
    let completed = Arc::new(AtomicUsize::new(0));
    let gateway = builder(Catalog)
        .layer(Guard {
            completed: Arc::clone(&completed),
        })
        .health(crate::middleware::builtin::Health::healthz())
        .build();

    let reply = collect(gateway.handle(get("/healthz")).await).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.headers["cache-control"], "no-store");
    assert_eq!(reply.json()["status"], "SERVING");
    assert_eq!(completed.load(Ordering::SeqCst), 0, "no interceptor ran");
}
