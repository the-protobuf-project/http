//! Request metrics.

use crate::middleware::{CallCx, Interceptor, Outcome};
use std::sync::Arc;
use std::time::Duration;

/// One completed call, as a metric.
///
/// Every field is bounded-cardinality on purpose. `template` rather than the
/// concrete path, and `code` rather than the error message: a label that can
/// take unbounded values is how a metrics backend gets taken down by a service
/// it was meant to observe.
#[derive(Debug, Clone)]
pub struct RequestMetric<'a> {
    /// The fully-qualified service name.
    pub service: &'a str,
    /// The fully-qualified method name.
    pub method: &'a str,
    /// The path template, e.g. `/v1/{name=artists/*}`.
    pub template: &'a str,
    /// The HTTP method.
    pub http_method: &'a str,
    /// The response status.
    pub status: u16,
    /// The canonical code name, `"OK"` on success.
    pub code: &'a str,
    /// How long the call took.
    pub latency: Duration,
}

/// Receives metrics.
///
/// An interface rather than a Prometheus dependency: which client a deployment
/// uses, and whether it even uses Prometheus, is not this crate's business.
/// go-grpc-middleware makes the same split with `providers/prometheus`.
pub trait MetricsSink: Send + Sync + 'static {
    /// Records one completed call.
    fn record(&self, metric: &RequestMetric<'_>);
}

/// Reports every call to a sink.
#[derive(Clone)]
pub struct Metrics {
    sink: Arc<dyn MetricsSink>,
}

impl Metrics {
    /// Builds the interceptor.
    #[must_use]
    pub fn new(sink: impl MetricsSink) -> Self {
        Self {
            sink: Arc::new(sink),
        }
    }
}

impl std::fmt::Debug for Metrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Metrics").finish_non_exhaustive()
    }
}

impl Interceptor for Metrics {
    fn name(&self) -> &'static str {
        "metrics"
    }

    fn on_complete(&self, cx: &CallCx<'_>, outcome: &Outcome<'_>) {
        self.sink.record(&RequestMetric {
            service: cx.route.service,
            method: cx.route.method,
            template: cx.route.template,
            http_method: cx.route.http_method.as_str(),
            status: outcome.status().as_u16(),
            code: outcome.code(),
            latency: cx.elapsed(),
        });
    }
}
