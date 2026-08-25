//! The route table and its resolution rules.

use super::{CaptureError, Route, split_path};
use std::borrow::Cow;

/// A compiled route table, emitted by `protoc-gen-http` already sorted
/// most-specific-first.
///
/// The generator has already rejected any table containing an unresolvable
/// ambiguity, so a linear scan in the emitted order is both correct and
/// complete: the first route that matches is the one that should serve. This is
/// the payoff of deciding precedence at build time — grpc-gateway resolves
/// overlapping patterns by registration order, at request time, silently.
#[derive(Debug, Clone)]
pub struct RouteTable {
    /// The routes, most specific first.
    routes: &'static [Route],

    /// Whether any route declares a custom verb.
    ///
    /// Cached because it decides whether a trailing `:` in a path is worth
    /// treating as a verb at all, and that question is asked on every request.
    has_verb_routes: bool,
}

/// What one path matched.
#[derive(Debug)]
pub struct RouteMatch<'a> {
    /// The winning route.
    pub route: &'static Route,

    /// The raw, still-encoded path segments the route matched.
    ///
    /// Kept undecoded so [`RouteMatch::captures`] can apply the `%2F` rule from
    /// README §1.2 per segment.
    pub segments: Vec<&'a str>,

    /// The AIP-136 custom verb, or `""`.
    pub verb: &'a str,
}

impl<'a> RouteMatch<'a> {
    /// Decodes every capture, as `(protojson name, value)` pairs.
    ///
    /// # Errors
    ///
    /// Returns the first [`CaptureError`] rather than a partial list: a
    /// malformed path is a `400`, and there is nothing useful left to bind.
    pub fn captures(&self) -> Result<Vec<(&'static str, Cow<'a, str>)>, CaptureError> {
        self.route
            .captures
            .iter()
            .map(|c| Ok((c.json, self.route.capture(c, &self.segments)?)))
            .collect()
    }
}

/// The outcome of resolving a request against the table.
///
/// The three variants map to the three routing failures in README §1.5
/// Keeping them distinct is the point: collapsing `MethodNotAllowed` into a
/// generic error is how grpc-gateway turns a `405` into a `501`.
#[derive(Debug)]
pub enum Resolution<'a> {
    /// A route matched.
    Matched(RouteMatch<'a>),

    /// The path matched, but not for this HTTP method.
    MethodNotAllowed {
        /// The methods bound to this path, for the mandatory `Allow` header.
        allow: Vec<&'static str>,
    },

    /// No route matched the path at all.
    NotFound,
}

impl RouteTable {
    /// Builds a table over a generated route slice.
    pub fn new(routes: &'static [Route]) -> Self {
        Self {
            routes,
            has_verb_routes: routes.iter().any(|r| !r.verb.is_empty()),
        }
    }

    /// Resolves an HTTP method and path against the table.
    ///
    /// A `:` is legal inside a resource id, so a peeled verb is a guess: the
    /// verb-bearing routes are tried first, and if none claims it the path is
    /// retried with the colon as data. A suffix no registered route asked for
    /// is never stripped.
    pub fn resolve<'a>(&self, method: &str, path: &'a str) -> Resolution<'a> {
        if self.has_verb_routes {
            let (segments, verb) = split_path(path, true);
            if !verb.is_empty() {
                match self.scan(method, segments, verb) {
                    Resolution::NotFound => {}
                    resolved => return resolved,
                }
            }
        }
        let (segments, _) = split_path(path, false);
        self.scan(method, segments, "")
    }

    /// Scans the table once, collecting the methods bound to a matching path so
    /// a `405` can name them.
    fn scan<'a>(&self, method: &str, segments: Vec<&'a str>, verb: &'a str) -> Resolution<'a> {
        let mut allow: Vec<&'static str> = Vec::new();

        for route in self.routes {
            if !route.matches(&segments, verb) {
                continue;
            }
            if route.method == method {
                return Resolution::Matched(RouteMatch {
                    route,
                    segments,
                    verb,
                });
            }
            if !allow.contains(&route.method) {
                allow.push(route.method);
            }
        }

        if allow.is_empty() {
            Resolution::NotFound
        } else {
            Resolution::MethodNotAllowed { allow }
        }
    }

    /// The routes in the table, in scan order.
    pub fn routes(&self) -> &'static [Route] {
        self.routes
    }
}
