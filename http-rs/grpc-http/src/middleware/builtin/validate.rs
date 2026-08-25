//! Payload validation.

use crate::error::{FieldViolation, GatewayError, Result};
use crate::middleware::{CallCx, InspectRequest};
use std::sync::Arc;

/// Checks a bound request message.
///
/// Generated per message from the four sources in README §2.1: AIP-203
/// field behaviour, AIP-122/123 resource patterns, `google.api.field_info`
/// formats, and protovalidate constraints. Three of the four compile to direct
/// code; only CEL needs an evaluator at runtime.
pub trait Validator<M>: Send + Sync + 'static {
    /// Collects every violation in `message`.
    ///
    /// Collecting rather than returning at the first is the whole point: a
    /// caller with three bad fields should learn about three, not discover them
    /// one round trip at a time.
    fn validate(&self, message: &M, out: &mut Vec<FieldViolation>);
}

/// Rejects an invalid request before the RPC is dialled.
///
/// This is what grpc-gateway has no place for. Its extension points all sit
/// either side of the message — `WithMetadata` before it exists,
/// `WithForwardResponseOption` after the call — so there is no hook that can
/// see a decoded request, and validation ends up in every service instead.
///
/// Gateway-side validation is defence in depth, not a substitute for the
/// service's own: a service must still assume unvalidated input, because the
/// gateway is not the only way in. What this buys is a good error at the edge
/// and a truthful `OpenAPI` document.
pub struct Validate<M> {
    validator: Arc<dyn Validator<M>>,
    domain: &'static str,
}

impl<M> Validate<M> {
    /// Builds the interceptor for one message type.
    #[must_use]
    pub fn new(validator: impl Validator<M>, domain: &'static str) -> Self {
        Self {
            validator: Arc::new(validator),
            domain,
        }
    }
}

impl<M> Clone for Validate<M> {
    fn clone(&self) -> Self {
        Self {
            validator: Arc::clone(&self.validator),
            domain: self.domain,
        }
    }
}

impl<M> std::fmt::Debug for Validate<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Validate")
            .field("domain", &self.domain)
            .finish()
    }
}

impl<M: Send + Sync + 'static> InspectRequest<M> for Validate<M> {
    fn inspect_request(&self, cx: &mut CallCx<'_>, message: &mut M) -> Result<()> {
        let mut violations = Vec::new();
        self.validator.validate(message, &mut violations);

        if violations.is_empty() {
            return Ok(());
        }
        Err(Box::new(GatewayError::invalid_fields(
            violations,
            "INVALID_ARGUMENT",
            self.domain,
            cx.route.method,
        )))
    }
}
