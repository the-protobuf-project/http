//! Interceptors that ship with the crate.
//!
//! The set mirrors [`go-grpc-middleware`], which is the closest thing the gRPC
//! ecosystem has to a canonical list, plus the two grpc-gateway provides as
//! mux options:
//!
//! | `go-grpc-middleware` | Here |
//! | --- | --- |
//! | `auth` | [`Auth`] |
//! | `logging` | [`Logging`] |
//! | `protovalidate` / `validator` | [`Validate`] |
//! | `ratelimit` | [`RateLimit`] |
//! | `realip` | [`RealIp`] |
//! | `recovery` | [`Recovery`] |
//! | `timeout` | [`Deadline`] |
//! | `selector` | [`Selector`](super::Selector), on every layer |
//! | `providers/prometheus` | [`Metrics`] |
//! | `retry` | not applicable — see below |
//!
//! `retry` is a *client* interceptor in go-grpc-middleware, and retrying at the
//! handler would be wrong: it cannot know whether a method is idempotent, and
//! replaying a non-idempotent one turns a timeout into a duplicate write. The
//! parts of it that do belong here are AIP-155 request-id deduplication
//! ([`Idempotency`]) and telling the client when to retry, which the AIP-193
//! `RetryInfo` detail already does.
//!
//! From grpc-gateway's mux options: [`Health`] covers `WithHealthzEndpoint` and
//! `WithHealthEndpointAt`, and [`Cors`] covers what its users reach for
//! `WithMiddlewares` to do.
//!
//! [`go-grpc-middleware`]: https://github.com/grpc-ecosystem/go-grpc-middleware

mod auth;
mod cors;
mod deadline;
mod health;
mod idempotency;
mod logging;
mod metrics;
mod ratelimit;
mod realip;
mod recovery;
mod validate;

pub use auth::{Auth, Authenticator, Identity};
pub use cors::{Cors, Origins};
pub use deadline::Deadline;
pub use health::{Health, ServingStatus};
pub use idempotency::{Idempotency, RequestIdStore};
pub use logging::Logging;
pub use metrics::{Metrics, MetricsSink, RequestMetric};
pub use ratelimit::{Limiter, RateLimit};
pub use realip::{ClientIp, RealIp};
pub use recovery::Recovery;
pub use validate::{Validate, Validator};
