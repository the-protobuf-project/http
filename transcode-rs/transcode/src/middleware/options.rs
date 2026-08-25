//! The handler's behavioural knobs.
//!
//! These are grpc-gateway's boolean `ServeMuxOption`s, gathered into one struct
//! rather than seventeen constructors. Each default is stated with why.

use std::time::Duration;

/// How much of a path is decoded before matching.
///
/// grpc-gateway's `UnescapingMode`, with one fewer mode: its `Legacy` default
/// unescapes everything *before* routing, which lets a `%2F` invent a segment
/// boundary and silently change which route matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UnescapingMode {
    /// Split on `/` first, then decode each captured segment, leaving `%2F`
    /// encoded. The default, and what README §1.2 requires.
    #[default]
    AfterMatch,
    /// Decode the whole path before matching, as grpc-gateway does by default.
    ///
    /// Offered only for migrating a service whose clients depend on it. It
    /// makes `artists/a%2Fb` and `artists/a/b` indistinguishable.
    BeforeMatch,
}

/// Transcoder behaviour that is not a policy.
#[derive(Debug, Clone)]
pub struct Options {
    /// How the path is decoded. See [`UnescapingMode`].
    pub unescaping: UnescapingMode,

    /// Retry a failed match against the path with its last segment removed.
    ///
    /// grpc-gateway's "path length fallback", which exists so a verb-bearing
    /// route can be reached by a client that split the verb wrongly. Off here:
    /// the verb handling in README §1.3 already retries the colon as
    /// data, and a second implicit retry makes the route a request reached
    /// unpredictable.
    pub path_length_fallback: bool,

    /// Honour `X-HTTP-Method-Override`.
    ///
    /// Off by default. It lets a `POST` become a `DELETE`, which is a real
    /// need behind restrictive proxies and a real hazard everywhere else, so it
    /// is opt-in rather than opt-out.
    pub method_override: bool,

    /// Write `Content-Length` on unary responses.
    ///
    /// On, because the body is fully encoded before the status line is written
    /// anyway (README §6.2), so the length is free and clients handle a
    /// known length better.
    pub write_content_length: bool,

    /// Use chunked transfer-encoding for streams on HTTP/1.1.
    ///
    /// On. HTTP/2 and HTTP/3 frame natively and ignore this.
    pub chunked_encoding: bool,

    /// Reject a request body on a binding that declares none.
    ///
    /// On, per README §2 A body nobody will read is far more likely
    /// to be a client mistake than an intention.
    pub reject_unexpected_body: bool,

    /// Reject query parameters that name no field.
    ///
    /// On, per README §2 grpc-gateway discards them, which turns a
    /// typo in an update call into a silent no-op.
    pub reject_unknown_query: bool,

    /// The default deadline when the client sends no `Grpc-Timeout`.
    ///
    /// Always set. An unbounded default turns one slow backend into
    /// connection-pool exhaustion, so the question is only what the number is.
    pub default_timeout: Duration,

    /// The largest request body accepted, in bytes.
    pub max_body_bytes: usize,

    /// Include `DebugInfo` details in error responses.
    ///
    /// Off. It can describe the shape of the service to a caller, and should
    /// only ever be on for a loopback listener.
    pub expose_debug_info: bool,

    /// The API's error domain, stamped into every AIP-193 `ErrorInfo`.
    pub domain: &'static str,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            unescaping: UnescapingMode::default(),
            path_length_fallback: false,
            method_override: false,
            write_content_length: true,
            chunked_encoding: true,
            reject_unexpected_body: true,
            reject_unknown_query: true,
            default_timeout: Duration::from_secs(30),
            max_body_bytes: 4 * 1024 * 1024,
            expose_debug_info: false,
            domain: "example.com",
        }
    }
}

impl Options {
    /// Options for a given API domain.
    #[must_use]
    pub fn new(domain: &'static str) -> Self {
        Self {
            domain,
            ..Self::default()
        }
    }

    /// Sets the default deadline.
    #[must_use]
    pub fn with_default_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// Sets the maximum request body size.
    #[must_use]
    pub fn with_max_body_bytes(mut self, bytes: usize) -> Self {
        self.max_body_bytes = bytes;
        self
    }

    /// Enables `DebugInfo` in error responses.
    ///
    /// # Panics
    ///
    /// Panics if called on options that are not loopback-only. Debug details
    /// describe the service's internals, and the failure mode of leaking them
    /// is bad enough to be worth refusing at construction rather than
    /// documenting.
    #[must_use]
    pub fn with_debug_info(mut self, loopback_only: bool) -> Self {
        assert!(
            loopback_only,
            "expose_debug_info requires a loopback-only listener"
        );
        self.expose_debug_info = true;
        self
    }
}
