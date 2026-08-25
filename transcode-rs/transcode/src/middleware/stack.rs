//! The interceptor chain.

use super::{CallCx, Interceptor, MethodPattern, Outcome, ResponseParts, RouteCx, Selector};
use crate::error::Result;
use std::sync::Arc;

/// One interceptor and the methods it applies to.
#[derive(Clone)]
pub struct Selected {
    interceptor: Arc<dyn Interceptor>,
    selector: Selector,
}

impl Selected {
    /// Whether this entry runs for a method.
    fn applies(&self, service: &str, method: &str, pattern: MethodPattern) -> bool {
        self.selector.matches(service, method, pattern)
    }
}

impl std::fmt::Debug for Selected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Selected")
            .field("interceptor", &self.interceptor.name())
            .field("selector", &self.selector)
            .finish()
    }
}

/// An ordered chain of interceptors.
///
/// Order is the order they were added: `on_route` and `on_request` run
/// forwards, `on_response` and `on_complete` run **backwards**, so a stack
/// nests the way a reader expects — the first interceptor added is the
/// outermost, and it sees the response last.
#[derive(Clone, Default, Debug)]
pub struct Stack {
    entries: Vec<Selected>,
}

impl Stack {
    /// An empty stack.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an interceptor for every method.
    #[must_use]
    pub fn layer(self, interceptor: impl Interceptor) -> Self {
        self.layer_on(interceptor, Selector::All)
    }

    /// Adds an interceptor for the methods a selector covers.
    ///
    /// `Selector::Mutating` resolves against the AIP pattern the generator
    /// emitted, so a policy written once keeps covering methods added later.
    #[must_use]
    pub fn layer_on(mut self, interceptor: impl Interceptor, selector: Selector) -> Self {
        self.entries.push(Selected {
            interceptor: Arc::new(interceptor),
            selector,
        });
        self
    }

    /// Runs the `on_route` phase, in order.
    ///
    /// # Errors
    ///
    /// The first rejection, which skips the rest of the chain.
    pub fn on_route(&self, cx: &mut RouteCx<'_>) -> Result<()> {
        for entry in &self.entries {
            if entry.applies(cx.service, cx.method, cx.pattern) {
                entry.interceptor.on_route(cx)?;
            }
        }
        Ok(())
    }

    /// Runs the `on_request` phase, in order.
    ///
    /// # Errors
    ///
    /// The first rejection.
    pub fn on_request(&self, cx: &mut CallCx<'_>) -> Result<()> {
        for entry in &self.entries {
            if entry.applies(cx.route.service, cx.route.method, cx.route.pattern) {
                entry.interceptor.on_request(cx)?;
            }
        }
        Ok(())
    }

    /// Runs the `on_response` phase, in reverse order.
    ///
    /// # Errors
    ///
    /// The first rejection.
    pub fn on_response(&self, cx: &mut CallCx<'_>, parts: &mut ResponseParts) -> Result<()> {
        for entry in self.entries.iter().rev() {
            if entry.applies(cx.route.service, cx.route.method, cx.route.pattern) {
                entry.interceptor.on_response(cx, parts)?;
            }
        }
        Ok(())
    }

    /// Runs the `on_complete` phase, in reverse order.
    ///
    /// Every interceptor that applies is called even if an earlier phase
    /// rejected, so a logger records the rejection it caused.
    pub fn on_complete(&self, cx: &CallCx<'_>, outcome: &Outcome<'_>) {
        for entry in self.entries.iter().rev() {
            if entry.applies(cx.route.service, cx.route.method, cx.route.pattern) {
                entry.interceptor.on_complete(cx, outcome);
            }
        }
    }

    /// The interceptor names, outermost first.
    #[must_use]
    pub fn names(&self) -> Vec<&'static str> {
        self.entries.iter().map(|e| e.interceptor.name()).collect()
    }
}
