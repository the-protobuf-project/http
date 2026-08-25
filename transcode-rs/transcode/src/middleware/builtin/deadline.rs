//! Deadlines.

use crate::error::Result;
use crate::middleware::metadata::parse_grpc_timeout;
use crate::middleware::{CallCx, Interceptor, RouteCx};
use std::time::Duration;

/// Bounds how long a call may run.
///
/// The deadline comes from, in order: the client's `Grpc-Timeout` header, a
/// per-method override, then the configured default. A transcoder must always
/// have one — an unbounded default turns a single slow backend into
/// connection-pool exhaustion, and by the time that is visible the cause is
/// several layers away.
///
/// A client asking for longer than [`Deadline::max`] is capped rather than
/// refused, since the request is otherwise perfectly valid.
#[derive(Debug, Clone)]
pub struct Deadline {
    default: Duration,
    max: Duration,
    /// The API's error domain, stamped into the `ErrorInfo` of an expiry.
    ///
    /// Carried rather than looked up because an `ErrorInfo` naming the wrong
    /// domain is worse than useless: a caller routes on it, so a literal like
    /// `"gateway"` sends them looking for a service that does not exist.
    domain: &'static str,
}

impl Deadline {
    /// A deadline with the given default and a ceiling of five minutes.
    #[must_use]
    pub const fn new(default: Duration, domain: &'static str) -> Self {
        Self {
            default,
            max: Duration::from_secs(300),
            domain,
        }
    }

    /// Sets the ceiling a client may request.
    #[must_use]
    pub const fn with_max(mut self, max: Duration) -> Self {
        self.max = max;
        self
    }

    /// Resolves the deadline for one request.
    #[must_use]
    pub fn resolve(&self, cx: &RouteCx<'_>) -> Duration {
        cx.headers
            .get("grpc-timeout")
            .and_then(|value| value.to_str().ok())
            .and_then(parse_grpc_timeout)
            .map_or(self.default, |requested| requested.min(self.max))
    }
}

impl Interceptor for Deadline {
    fn name(&self) -> &'static str {
        "deadline"
    }

    /// Forwards the resolved deadline to the service as `grpc-timeout`, so the
    /// backend stops working on a call the transcoder has already abandoned.
    fn on_route(&self, cx: &mut RouteCx<'_>) -> Result<()> {
        let deadline = self.resolve(cx);
        cx.metadata
            .append("grpc-timeout", format!("{}m", deadline.as_millis()));
        Ok(())
    }

    /// Fails the call if the deadline passed while it was in flight.
    fn on_request(&self, cx: &mut CallCx<'_>) -> Result<()> {
        if cx.expired() {
            return Err(cx.deadline_error(self.domain));
        }
        Ok(())
    }
}
