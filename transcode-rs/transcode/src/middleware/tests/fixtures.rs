//! Shared scaffolding for the middleware tests.

use crate::middleware::{CallCx, Metadata, MethodPattern, RouteCx};
use http::{HeaderMap, Method, Uri};
use std::collections::HashMap;
use std::time::Instant;

/// The API domain used throughout.
pub const DOMAIN: &str = "music.example.com";

/// Owns everything a [`RouteCx`] borrows, since the context is all references.
pub struct Fixture {
    /// The HTTP method.
    pub method: Method,
    /// The request URI.
    pub uri: Uri,
    /// The request headers.
    pub headers: HeaderMap,
    /// Path captures.
    pub captures: HashMap<&'static str, String>,
}

impl Fixture {
    /// A `GET /v1/artists/miles` against `ArtistService.GetArtist`.
    pub fn get() -> Self {
        Self {
            method: Method::GET,
            uri: "/v1/artists/miles".parse().unwrap(),
            headers: HeaderMap::new(),
            captures: HashMap::new(),
        }
    }

    /// A `POST /v1/artists`.
    pub fn post() -> Self {
        Self {
            method: Method::POST,
            uri: "/v1/artists".parse().unwrap(),
            headers: HeaderMap::new(),
            captures: HashMap::new(),
        }
    }

    /// Sets a request header.
    pub fn header(mut self, name: &'static str, value: &str) -> Self {
        self.headers
            .insert(name, http::HeaderValue::from_str(value).unwrap());
        self
    }

    /// Builds a routing context for a method and its AIP pattern.
    pub fn route(&self, method: &'static str, pattern: MethodPattern) -> RouteCx<'_> {
        RouteCx {
            http_method: &self.method,
            uri: &self.uri,
            headers: &self.headers,
            peer: Some("203.0.113.9:44321".parse().unwrap()),
            service: "music.v1.ArtistService",
            method,
            pattern,
            template: "/v1/{name=artists/*}",
            captures: &self.captures,
            metadata: Metadata::new(),
            extensions: crate::middleware::context::Extensions::new(),
        }
    }

    /// Builds a call context.
    pub fn call(&self, method: &'static str, pattern: MethodPattern) -> CallCx<'_> {
        CallCx {
            route: self.route(method, pattern),
            started: Instant::now(),
            deadline: None,
        }
    }
}
