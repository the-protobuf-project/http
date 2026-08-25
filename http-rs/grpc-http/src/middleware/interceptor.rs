//! The interceptor traits.

use super::{CallCx, Outcome, ResponseParts, RouteCx};
use crate::error::Result;

/// Runs around a call, without seeing its payload.
///
/// Object-safe on purpose: the registry holds `Arc<dyn Interceptor>`, and this
/// covers the majority of real policies, because authn, authz, quota, audit,
/// and tracing all key on *which* method was called rather than on what it was
/// sent. Payload access is the specialisation, not the default — see
/// [`InspectRequest`].
///
/// Every method has a default, so an implementation names only the phases it
/// cares about.
pub trait Interceptor: Send + Sync + 'static {
    /// A name, for tracing and for diagnosing a stack.
    fn name(&self) -> &'static str;

    /// After routing, before the body is read.
    ///
    /// The right place to reject: nothing has been decoded, so a `401` here
    /// costs nothing. Returning `Err` skips the call and every later phase
    /// except [`Interceptor::on_complete`].
    ///
    /// # Errors
    ///
    /// Whatever the policy decides — typically `401`, `403`, or `429`.
    fn on_route(&self, cx: &mut RouteCx<'_>) -> Result<()> {
        let _ = cx;
        Ok(())
    }

    /// After the request message is bound and validated, before the RPC.
    ///
    /// # Errors
    ///
    /// Whatever the policy decides.
    fn on_request(&self, cx: &mut CallCx<'_>) -> Result<()> {
        let _ = cx;
        Ok(())
    }

    /// After the RPC returns, before the response is encoded.
    ///
    /// This is grpc-gateway's `WithForwardResponseOption`: the place to set a
    /// header or change the status from what the handler chose.
    ///
    /// # Errors
    ///
    /// Turns the response into a failure, which is how a response-side policy
    /// rejects.
    fn on_response(&self, cx: &mut CallCx<'_>, parts: &mut ResponseParts) -> Result<()> {
        let _ = (cx, parts);
        Ok(())
    }

    /// After everything, success or failure.
    ///
    /// Cannot fail and cannot change the response — it has already been
    /// written. For logging, metrics, and audit.
    fn on_complete(&self, cx: &CallCx<'_>, outcome: &Outcome<'_>) {
        let _ = (cx, outcome);
    }
}

/// Reads or rewrites a typed request message.
///
/// Not object-safe, and deliberately: the generated handler knows `M`, so it
/// monomorphises the call. This is the opt-in half of the message plane, for
/// the policies that genuinely need the payload — redacting a field, enforcing
/// a cross-field invariant, stamping a server-side default.
pub trait InspectRequest<M>: Send + Sync + 'static {
    /// Inspects or rewrites the bound request.
    ///
    /// # Errors
    ///
    /// Rejects the call, typically with `400` or `403`.
    fn inspect_request(&self, cx: &mut CallCx<'_>, message: &mut M) -> Result<()>;
}

/// Reads or rewrites a typed response message.
///
/// grpc-gateway's `WithForwardResponseRewriter`, with the type intact: that hook
/// receives a `proto.Message` and returns `any`, so a rewriter has to type-switch
/// at runtime and can return something the marshaler then fails on.
pub trait InspectResponse<M>: Send + Sync + 'static {
    /// Inspects or rewrites the response before encoding.
    ///
    /// # Errors
    ///
    /// Turns the response into a failure.
    fn inspect_response(&self, cx: &mut CallCx<'_>, message: &mut M) -> Result<()>;
}
