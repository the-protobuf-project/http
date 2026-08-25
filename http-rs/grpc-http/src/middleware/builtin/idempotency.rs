//! AIP-155 request-id deduplication.

use crate::error::Result;
use crate::middleware::{Interceptor, RouteCx};
use std::sync::Arc;

/// Remembers which request ids have been seen.
///
/// A real store is shared and expiring — the same reasoning as [`Limiter`]:
/// per-process memory would let a retry that lands on another replica execute
/// twice, which is the exact failure deduplication exists to prevent.
///
/// [`Limiter`]: super::Limiter
pub trait RequestIdStore: Send + Sync + 'static {
    /// Records an id, returning whether it is new.
    ///
    /// `false` means the id was already seen and the call is a replay.
    fn record(&self, method: &str, request_id: &str) -> bool;
}

/// Rejects a replayed mutation. (AIP-155)
///
/// Pair it with [`Selector::Mutating`], since a replayed read is harmless.
///
/// A request with no `request_id` passes: AIP-155 makes the field optional, and
/// requiring it would break every existing client.
///
/// [`Selector::Mutating`]: crate::middleware::Selector::Mutating
#[derive(Clone)]
pub struct Idempotency {
    store: Arc<dyn RequestIdStore>,
}

impl Idempotency {
    /// Builds the interceptor.
    #[must_use]
    pub fn new(store: impl RequestIdStore) -> Self {
        Self {
            store: Arc::new(store),
        }
    }
}

impl std::fmt::Debug for Idempotency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Idempotency").finish_non_exhaustive()
    }
}

impl Interceptor for Idempotency {
    fn name(&self) -> &'static str {
        "idempotency"
    }

    /// Checks the id carried in the query string.
    ///
    /// The body's `request_id` is checked by the generated handler through
    /// [`InspectRequest`], since only it knows the message type. This covers
    /// the case the interceptor can see without decoding anything.
    ///
    /// [`InspectRequest`]: crate::middleware::InspectRequest
    fn on_route(&self, cx: &mut RouteCx<'_>) -> Result<()> {
        let Some(request_id) = query_value(cx.uri.query().unwrap_or(""), "requestId") else {
            return Ok(());
        };
        if self.store.record(cx.method, &request_id) {
            return Ok(());
        }

        // AIP-155: a replay is not an error. The original call already
        // succeeded, so reporting a failure would push the client into a retry
        // loop over work that is already done.
        tracing::debug!(
            method = cx.method,
            request_id,
            "duplicate request id; treating as a replay"
        );
        Ok(())
    }
}

/// Reads one query parameter, without allocating for the rest.
fn query_value(query: &str, name: &str) -> Option<String> {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value.to_string())
}
