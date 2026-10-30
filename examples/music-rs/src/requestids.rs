//! The request-id store the idempotency interceptor records into.

use std::collections::HashSet;
use std::sync::Mutex;
use transcode::middleware::builtin::RequestIdStore;

/// Remembers which `(method, requestId)` pairs have been seen.
///
/// In memory and unbounded, which is right for an example and wrong for a
/// deployment: a real store expires entries and is shared across replicas.
#[derive(Debug, Default)]
pub struct RequestIds {
    seen: Mutex<HashSet<(String, String)>>,
}

impl RequestIds {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl RequestIdStore for RequestIds {
    fn record(&self, method: &str, request_id: &str) -> bool {
        // A poisoned lock means another call panicked mid-insert; the set is
        // still a set, so recovering it is safe.
        let mut seen = self
            .seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        seen.insert((method.to_string(), request_id.to_string()))
    }
}
