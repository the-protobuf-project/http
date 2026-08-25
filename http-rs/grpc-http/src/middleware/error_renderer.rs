//! Turning a failure into a response.

use super::{CallCx, Options, RouteCx};
use crate::error::GatewayError;

/// Renders an error.
///
/// This replaces three grpc-gateway hooks — `WithErrorHandler`,
/// `WithStreamErrorHandler`, and `WithRoutingErrorHandler` — with one. Having
/// three is how its unary errors, stream errors, and routing errors ended up
/// disagreeing about both status and body shape; a single funnel makes that
/// class of divergence unrepresentable.
pub trait ErrorRenderer: Send + Sync + 'static {
    /// Adjusts an error before it is written.
    ///
    /// Called for every failure regardless of where it arose. `route` is
    /// `None` when the failure happened before a route matched, which is
    /// exactly the case grpc-gateway routes through a separate handler.
    fn render(&self, err: &mut GatewayError, route: Option<&RouteCx<'_>>, options: &Options);

    /// Observes a failure on a call that had already started.
    ///
    /// Separate from [`ErrorRenderer::render`] because a stream that fails
    /// after its first message cannot have its status changed — the header is
    /// spent — but the failure must still be recorded.
    fn observe(&self, err: &GatewayError, cx: &CallCx<'_>) {
        let _ = (err, cx);
    }
}

/// The default renderer.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultErrorRenderer;

impl ErrorRenderer for DefaultErrorRenderer {
    /// Guarantees the AIP-193 invariants: an `ErrorInfo` is present, and
    /// `DebugInfo` is stripped unless explicitly allowed.
    fn render(&self, err: &mut GatewayError, route: Option<&RouteCx<'_>>, options: &Options) {
        if !options.expose_debug_info {
            err.details
                .retain(|d| !matches!(d, crate::error::Detail::DebugInfo(_)));
        }

        // A service that returned no ErrorInfo still gets one, and when a route
        // matched it names the method — which is the first thing an operator
        // reading the error wants to know.
        if !err
            .details
            .iter()
            .any(|d| matches!(d, crate::error::Detail::ErrorInfo(_)))
        {
            let metadata = route
                .map(|r| vec![("method".to_string(), r.method.to_string())])
                .unwrap_or_default();
            err.details.insert(
                0,
                crate::error::Detail::ErrorInfo(crate::error::ErrorInfo {
                    reason: err.code.as_str().to_string(),
                    domain: options.domain.to_string(),
                    metadata: metadata.into_iter().collect(),
                }),
            );
        }
    }

    /// Logs at a level matching how actionable the failure is: a `5xx` is the
    /// service's problem, a `4xx` is the caller's.
    fn observe(&self, err: &GatewayError, cx: &CallCx<'_>) {
        if err.http.is_server_error() {
            tracing::error!(
                method = cx.route.method,
                status = err.http.as_u16(),
                code = err.code.as_str(),
                message = %err.message,
                "call failed"
            );
        } else {
            tracing::debug!(
                method = cx.route.method,
                status = err.http.as_u16(),
                code = err.code.as_str(),
                "call rejected"
            );
        }
    }
}
