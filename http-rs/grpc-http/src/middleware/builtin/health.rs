//! Health endpoints.

use crate::middleware::ResponseParts;
use http::StatusCode;

/// What `grpc.health.v1.Health` reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ServingStatus {
    /// The service is up.
    #[default]
    Serving,
    /// The service is down. Answered as `503`, so a load balancer acts on it
    /// without reading the body.
    NotServing,
    /// The service is unknown to the health server, which is `404`: reporting
    /// `503` would imply it exists and is merely down.
    ServiceUnknown,
}

impl ServingStatus {
    /// The HTTP status this maps to.
    #[must_use]
    pub const fn http_status(self) -> StatusCode {
        match self {
            ServingStatus::Serving => StatusCode::OK,
            ServingStatus::NotServing => StatusCode::SERVICE_UNAVAILABLE,
            ServingStatus::ServiceUnknown => StatusCode::NOT_FOUND,
        }
    }

    /// The name the health protocol uses.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            ServingStatus::Serving => "SERVING",
            ServingStatus::NotServing => "NOT_SERVING",
            ServingStatus::ServiceUnknown => "SERVICE_UNKNOWN",
        }
    }
}

/// Serves a health endpoint.
///
/// grpc-gateway's `WithHealthzEndpoint` and `WithHealthEndpointAt`. It answers
/// before routing, so a health check keeps working when the route table cannot
/// serve anything else — which is precisely when a health check matters.
#[derive(Debug, Clone)]
pub struct Health {
    path: &'static str,
    checker: fn(Option<&str>) -> ServingStatus,
}

impl Health {
    /// A health endpoint at `/healthz` that always reports serving.
    #[must_use]
    pub fn healthz() -> Self {
        Self {
            path: "/healthz",
            checker: |_| ServingStatus::Serving,
        }
    }

    /// A health endpoint at a chosen path, backed by a checker.
    ///
    /// The checker receives the `?service=` parameter, matching the
    /// `grpc.health.v1.Health/Check` request field.
    #[must_use]
    pub fn at(path: &'static str, checker: fn(Option<&str>) -> ServingStatus) -> Self {
        Self { path, checker }
    }

    /// Whether this request is a health check.
    #[must_use]
    pub fn handles(&self, path: &str) -> bool {
        path == self.path
    }

    /// Answers a health check.
    ///
    /// Returns the response parts and the body, which is the same JSON shape
    /// `grpc.health.v1.Health` returns over gRPC.
    #[must_use]
    pub fn respond(&self, query: Option<&str>) -> (ResponseParts, Vec<u8>) {
        let service = query
            .and_then(|q| {
                q.split('&')
                    .filter_map(|pair| pair.split_once('='))
                    .find(|(key, _)| *key == "service")
                    .map(|(_, value)| value)
            })
            .filter(|value| !value.is_empty());

        let status = (self.checker)(service);
        let mut parts = ResponseParts::ok();
        parts.status = status.http_status();
        parts.set(http::header::CONTENT_TYPE, "application/json");
        // Health checks are polled constantly and must never be cached.
        parts.set(http::header::CACHE_CONTROL, "no-store");

        let body = format!("{{\"status\":\"{}\"}}", status.as_str()).into_bytes();
        (parts, body)
    }
}
