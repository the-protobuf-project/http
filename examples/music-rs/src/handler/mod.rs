//! The request pipeline: route, bind, call, encode.
//!
//! This is what `protoc-gen-http` will emit per service. Writing it by hand
//! first fixes the shape the generator has to produce, and proves the runtime
//! works end to end before the generator exists.
//!
//! The order is the one README §2 fixes, and every stage funnels its
//! failure through the same [`Error`], which is the property
//! grpc-gateway lacks:
//!
//! ```text
//! route ─► 404 / 405        bind ─► 400 / 415        call ─► mapped Status
//!                                                  encode ─► 406 / 500
//! ```

use crate::generated::{DOMAIN, Method, ROUTES};
use crate::store::Catalog;
use http::{HeaderMap, Method as HttpMethod, StatusCode};
use std::sync::Arc;
use transcode::error::Error;
use transcode::route::{Resolution, RouteTable};

mod artists;
mod call;
mod dispatch;
mod negotiate;
mod query;
mod reply;
mod tracks;
mod watch;

pub use call::Call;
pub use negotiate::Codecs;
use query::{malformed_path, parse_query};
pub use watch::FAIL_AFTER;

/// The handler: a route table, a codec registry, and the service behind it.
#[derive(Clone, Debug)]
pub struct Handler {
    routes: RouteTable,
    catalog: Arc<Catalog>,
}

/// A rendered HTTP response.
#[derive(Debug)]
pub struct Reply {
    /// The status line.
    pub status: StatusCode,
    /// Response headers, including any projected from error details.
    pub headers: HeaderMap,
    /// The encoded body.
    pub body: Vec<u8>,
}

impl Handler {
    /// Builds a handler over a catalog.
    #[must_use]
    pub fn new(catalog: Arc<Catalog>) -> Self {
        Self {
            routes: RouteTable::new(ROUTES),
            catalog,
        }
    }

    /// The route table, for tests that assert on resolution directly.
    #[must_use]
    pub fn routes(&self) -> &RouteTable {
        &self.routes
    }

    /// Serves one request.
    ///
    /// Every failure — routing, decoding, the RPC's own status — leaves through
    /// [`Self::render_error`], so a `404` and a mid-call `PERMISSION_DENIED`
    /// produce the same envelope shape. That single funnel is the structural
    /// fix for grpc-gateway rendering its three error paths differently.
    pub fn serve(&self, method: &HttpMethod, uri: &str, body: Vec<u8>) -> Reply {
        self.serve_with(method, uri, body, None)
    }

    /// Serves one request, with the negotiation headers the transport read.
    pub fn serve_with(
        &self,
        method: &HttpMethod,
        uri: &str,
        body: Vec<u8>,
        accept: Option<&str>,
    ) -> Reply {
        self.serve_negotiated(method, uri, body, accept, None)
    }

    /// Serves one request, given both negotiation headers.
    ///
    /// `Content-Type` is separate from `Accept` because they answer different
    /// questions — what the body is, and what the caller will take — and a
    /// transport that reads one must be able to pass it without inventing the
    /// other.
    pub fn serve_negotiated(
        &self,
        method: &HttpMethod,
        uri: &str,
        body: Vec<u8>,
        accept: Option<&str>,
        content_type: Option<&str>,
    ) -> Reply {
        let (path, query_string) = uri.split_once('?').unwrap_or((uri, ""));

        match self.routes.resolve(method.as_str(), path) {
            Resolution::Matched(matched) => {
                let captures = match matched.captures() {
                    Ok(captures) => captures,
                    // The path matched but a captured segment is malformed, so
                    // this is a 400 rather than a 404.
                    Err(err) => {
                        return self.render_error(&malformed_path(&err, path));
                    }
                };

                let method = Method::from_handler(matched.route.handler);

                // Negotiated before the body is looked at, so a caller whose
                // Content-Type names no codec gets a 415 rather than whatever
                // the JSON decoder happens to say about bytes it cannot read.
                let codecs = match negotiate::negotiate(
                    content_type,
                    accept,
                    query::alt(query_string).as_deref(),
                    method.is_streaming(),
                ) {
                    Ok(codecs) => codecs,
                    Err(err) => return self.render_error(&err),
                };

                let call = Call {
                    catalog: &self.catalog,
                    path: captures
                        .into_iter()
                        .map(|(name, value)| (name, value.into_owned()))
                        .collect(),
                    query: parse_query(query_string),
                    body,
                    method,
                    accept: accept.map(ToString::to_string),
                    codecs,
                };

                match dispatch::dispatch(&call) {
                    Ok(reply) => reply,
                    Err(err) => self.render_error(&err),
                }
            }
            Resolution::MethodNotAllowed { allow } => {
                self.render_error(&Error::method_not_allowed(method.as_str(), &allow, DOMAIN))
            }
            Resolution::NotFound => self.render_error(&Error::route_not_found(path, DOMAIN)),
        }
    }

    /// Renders an error as an AIP-193 response.
    fn render_error(&self, err: &Error) -> Reply {
        let body = serde_json::to_vec(&err.to_json())
            .unwrap_or_else(|_| br#"{"error":{"code":500,"status":"INTERNAL"}}"#.to_vec());

        let mut headers = err.headers();
        headers.insert(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("application/json"),
        );
        Reply {
            status: err.http,
            headers,
            body,
        }
    }
}
