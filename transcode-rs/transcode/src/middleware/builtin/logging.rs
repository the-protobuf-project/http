//! Structured request logging.

use crate::middleware::{CallCx, Interceptor, Outcome};

/// Logs one line per completed call.
///
/// Labels use the *template* rather than the concrete path, so a log aggregator
/// groups `/v1/artists/miles` and `/v1/artists/coltrane` under
/// `/v1/{name=artists/*}` instead of treating every resource name as its own
/// event.
///
/// Nothing from the request body or the query string is logged. Those carry
/// caller data, and a log line is exactly the wrong place for it to end up.
#[derive(Debug, Clone, Copy)]
pub struct Logging {
    slow_threshold_ms: u128,
}

impl Logging {
    /// Logs every call, warning on those over 1s.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            slow_threshold_ms: 1_000,
        }
    }

    /// Sets the latency above which a successful call is logged as slow.
    #[must_use]
    pub const fn slow_after_ms(mut self, millis: u128) -> Self {
        self.slow_threshold_ms = millis;
        self
    }
}

impl Default for Logging {
    fn default() -> Self {
        Self::new()
    }
}

impl Interceptor for Logging {
    fn name(&self) -> &'static str {
        "logging"
    }

    /// Emits the access log line.
    ///
    /// Level follows what the reader can act on: a `5xx` is the service's
    /// problem, a slow success is worth noticing, and everything else is
    /// routine.
    fn on_complete(&self, cx: &CallCx<'_>, outcome: &Outcome<'_>) {
        let elapsed_ms = cx.elapsed().as_millis();
        let status = outcome.status().as_u16();
        let code = outcome.code();
        let method = cx.route.method;
        let template = cx.route.template;
        let http_method = cx.route.http_method.as_str();

        match outcome {
            Outcome::Failure(err) if err.http.is_server_error() => {
                tracing::error!(
                    method, template, http_method, status, code, elapsed_ms,
                    message = %err.message,
                    "request failed"
                );
            }
            Outcome::Failure(_) => {
                tracing::info!(
                    method,
                    template,
                    http_method,
                    status,
                    code,
                    elapsed_ms,
                    "request rejected"
                );
            }
            Outcome::Success(_) if elapsed_ms >= self.slow_threshold_ms => {
                tracing::warn!(
                    method,
                    template,
                    http_method,
                    status,
                    code,
                    elapsed_ms,
                    "request completed slowly"
                );
            }
            Outcome::Success(_) => {
                tracing::info!(
                    method,
                    template,
                    http_method,
                    status,
                    code,
                    elapsed_ms,
                    "request completed"
                );
            }
        }
    }
}
