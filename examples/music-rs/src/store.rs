//! An in-memory catalog.
//!
//! Stands in for the gRPC service the handler would normally call. It is
//! deliberately a plain synchronous store behind a mutex: the point of the
//! proof of concept is the HTTP surface, and a real backend would only obscure
//! whether that surface is correct.
//!
//! Errors are `tonic::Status`, exactly as a real service returns, so the
//! handler's error mapping is exercised for real rather than simulated.

use crate::model::{Artist, Track};
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use tonic::Status;

/// The catalog.
///
/// `BTreeMap` rather than `HashMap` so listing is ordered by resource name,
/// which makes responses deterministic and the tests meaningful.
#[derive(Debug, Default)]
pub struct Catalog {
    inner: Mutex<Inner>,
    /// Monotonic source for `etag` values.
    revision: AtomicU64,
}

/// The catalog's contents, guarded by one lock.
#[derive(Debug, Default)]
struct Inner {
    /// Artists keyed by resource name, `artists/{artist}`.
    artists: BTreeMap<String, Artist>,
    /// Tracks keyed by resource name, `artists/{artist}/tracks/{track}`.
    tracks: BTreeMap<String, Track>,
}

impl Catalog {
    /// An empty catalog.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A catalog with two artists and three tracks, for the example server and
    /// the tests.
    #[must_use]
    pub fn seeded() -> Self {
        let catalog = Self::new();
        catalog.seed("artists/miles", "Miles Davis", 4_312_000);
        catalog.seed("artists/coltrane", "John Coltrane", 2_871_003);
        catalog.seed_track("artists/miles/tracks/so-what", "So What", "545s");
        catalog.seed_track(
            "artists/miles/tracks/blue-in-green",
            "Blue in Green",
            "337s",
        );
        catalog.seed_track("artists/coltrane/tracks/giant-steps", "Giant Steps", "286s");
        catalog
    }

    /// Inserts one artist during seeding.
    fn seed(&self, name: &str, display_name: &str, listeners: i64) {
        let _ = self.create_artist(Artist {
            name: name.to_string(),
            display_name: display_name.to_string(),
            // Written as a string because that is how protojson carries an
            // int64, and the store speaks the same shape the wire does.
            monthly_listeners: listeners.to_string(),
            ..Default::default()
        });
    }

    /// Inserts one track during seeding.
    fn seed_track(&self, name: &str, title: &str, duration: &str) {
        let parent = name.rsplit_once("/tracks/").map(|(p, _)| p).unwrap_or("");
        let _ = self.create_track(
            parent,
            Track {
                name: name.to_string(),
                title: title.to_string(),
                duration: duration.to_string(),
                availability: crate::model::Availability::Streaming,
                ..Default::default()
            },
        );
    }

    /// The next `etag` value.
    fn next_etag(&self) -> String {
        format!("\"{}\"", self.revision.fetch_add(1, Ordering::Relaxed))
    }

    /// Takes the lock, converting a poisoned mutex into an internal error
    /// rather than propagating a panic into the request path.
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Inner>, Status> {
        self.inner
            .lock()
            .map_err(|_| Status::internal("catalog lock poisoned"))
    }
}

/// A fixed timestamp, so responses are byte-stable across runs and the golden
/// tests mean something. A real service would use the clock.
pub const FIXED_TIME: &str = "2026-08-24T12:00:00Z";

mod artists;
mod tracks;
