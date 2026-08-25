//! Turning a panic into a response.

use crate::error::GatewayError;
use crate::middleware::{CallCx, Interceptor, Outcome};

/// Catches an unwind and renders it as `500`.
///
/// A panic in one handler must not take down the connection, because on HTTP/2
/// and HTTP/3 that connection is carrying other people's requests. It also must
/// not reach the client: a panic payload frequently contains a file path, a
/// slice index, or a fragment of the data that caused it.
///
/// This interceptor records the panic. The catching itself happens in the
/// generated handler, which is the only place that owns the call — an
/// interceptor cannot wrap a future it never sees.
#[derive(Debug, Clone, Copy, Default)]
pub struct Recovery;

impl Recovery {
    /// Builds the interceptor.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Interceptor for Recovery {
    fn name(&self) -> &'static str {
        "recovery"
    }

    /// Logs a caught panic at error level, with the method that caused it.
    fn on_complete(&self, cx: &CallCx<'_>, outcome: &Outcome<'_>) {
        if let Outcome::Failure(err) = outcome
            && is_panic(err)
        {
            tracing::error!(
                method = cx.route.method,
                template = cx.route.template,
                elapsed_ms = cx.elapsed().as_millis(),
                "handler panicked; returned 500"
            );
        }
    }
}

/// Whether an error came from a caught panic.
fn is_panic(err: &GatewayError) -> bool {
    err.details.iter().any(|detail| {
        matches!(detail, crate::error::Detail::ErrorInfo(info) if info.reason == "GATEWAY_PANIC")
    })
}
