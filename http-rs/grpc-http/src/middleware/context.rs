//! What an interceptor sees.

use super::{Metadata, MethodPattern};
use crate::error::{Code, GatewayError};
use http::{HeaderMap, HeaderValue, Method, StatusCode, Uri};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

/// What is known once a route has matched, before the body is read.
///
/// This is the phase most policies run in: authn, authz, quota, and audit all
/// key on *which method* was reached, not on what it was sent.
#[derive(Debug)]
pub struct RouteCx<'a> {
    /// The HTTP method.
    pub http_method: &'a Method,
    /// The request URI, path and query.
    pub uri: &'a Uri,
    /// The request headers, as received.
    pub headers: &'a HeaderMap,
    /// The peer address, when the transport exposes one.
    ///
    /// `None` behind a proxy that does not pass it through; use
    /// [`builtin::RealIp`] to recover the original from `X-Forwarded-For`.
    ///
    /// [`builtin::RealIp`]: super::builtin::RealIp
    pub peer: Option<SocketAddr>,
    /// The fully-qualified service name, e.g. `music.v1.ArtistService`.
    pub service: &'static str,
    /// The fully-qualified method name.
    pub method: &'static str,
    /// The AIP classification, for [`Selector`](super::Selector).
    pub pattern: MethodPattern,
    /// The matched path template, e.g. `/v1/{name=artists/*}`.
    ///
    /// grpc-gateway exposes this as `HTTPPathPattern(ctx)`. It is the right
    /// label for a metric, because it has bounded cardinality where the
    /// concrete path does not.
    pub template: &'static str,
    /// Path captures, keyed by protojson field path.
    pub captures: &'a HashMap<&'static str, String>,
    /// Metadata to forward to the service, which an annotator may extend.
    pub metadata: Metadata,
    /// Arbitrary values passed between interceptors.
    pub extensions: Extensions,
}

/// A call in progress: everything in [`RouteCx`] plus timing and the deadline.
#[derive(Debug)]
pub struct CallCx<'a> {
    /// The routing context.
    pub route: RouteCx<'a>,
    /// When the call started, for latency measurement.
    pub started: Instant,
    /// The deadline, from `Grpc-Timeout` or configuration.
    pub deadline: Option<Duration>,
}

impl CallCx<'_> {
    /// How long the call has been running.
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// Whether the deadline has passed.
    #[must_use]
    pub fn expired(&self) -> bool {
        self.deadline.is_some_and(|limit| self.elapsed() >= limit)
    }

    /// The `DEADLINE_EXCEEDED` error for an expired call.
    #[must_use]
    pub fn deadline_error(&self, domain: &str) -> Box<GatewayError> {
        Box::new(
            GatewayError::new(
                Code::DeadlineExceeded,
                "The deadline expired before the operation could complete.",
            )
            .with_error_info(
                "DEADLINE_EXCEEDED",
                domain,
                [("method".into(), self.route.method.into())],
            ),
        )
    }
}

/// The response, before it is written.
///
/// This is what [`Interceptor::on_response`] mutates, and it is the counterpart
/// of grpc-gateway's `WithForwardResponseOption`.
///
/// [`Interceptor::on_response`]: super::Interceptor::on_response
#[derive(Debug)]
pub struct ResponseParts {
    /// The status line. An interceptor may change it — an AIP-133 Create
    /// promoting `200` to `201`, for instance.
    pub status: StatusCode,
    /// Response headers.
    pub headers: HeaderMap,
    /// Trailers, emitted when the client advertised `TE: trailers`.
    pub trailers: HeaderMap,
}

impl ResponseParts {
    /// An empty `200`.
    #[must_use]
    pub fn ok() -> Self {
        Self {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            trailers: HeaderMap::new(),
        }
    }

    /// Sets a header, ignoring a value the header grammar rejects.
    pub fn set(&mut self, name: http::header::HeaderName, value: &str) {
        if let Ok(value) = HeaderValue::from_str(value) {
            self.headers.insert(name, value);
        }
    }
}

/// How a call ended, for [`Interceptor::on_complete`].
///
/// [`Interceptor::on_complete`]: super::Interceptor::on_complete
#[derive(Debug)]
pub enum Outcome<'a> {
    /// The call succeeded with this status.
    Success(StatusCode),
    /// The call failed.
    Failure(&'a GatewayError),
}

impl Outcome<'_> {
    /// The HTTP status either way, for a metric label.
    #[must_use]
    pub fn status(&self) -> StatusCode {
        match self {
            Outcome::Success(status) => *status,
            Outcome::Failure(err) => err.http,
        }
    }

    /// The canonical code name, `"OK"` on success.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Outcome::Success(_) => "OK",
            Outcome::Failure(err) => err.code.as_str(),
        }
    }
}

/// A typed side-channel between interceptors.
///
/// An authenticator puts the caller's identity here and an authorizer reads it,
/// without either knowing about the other or the two having to agree on a
/// concrete context type.
#[derive(Default)]
pub struct Extensions {
    map: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl Extensions {
    /// An empty set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores a value, replacing any previous one of the same type.
    pub fn insert<T: Any + Send + Sync>(&mut self, value: T) {
        self.map.insert(TypeId::of::<T>(), Box::new(value));
    }

    /// Borrows a stored value.
    #[must_use]
    pub fn get<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.map.get(&TypeId::of::<T>())?.downcast_ref()
    }
}

impl std::fmt::Debug for Extensions {
    /// Prints the count rather than the values, which are opaque and may hold
    /// credentials.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Extensions")
            .field("len", &self.map.len())
            .finish()
    }
}
