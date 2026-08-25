//! Listeners.
//!
//! The point of this module is how little there is in it. [`Gateway::serve`]
//! is a plain function from a method, a URI, and a body to a [`Reply`] — it
//! knows nothing about connections, TLS, or protocol versions. Each listener
//! below is a small adapter that reads a request off its own transport, calls
//! that one function, and writes the answer back.
//!
//! That is the concrete payoff of the `tower::Service` shape in
//! the README: an HTTP/3 gateway is the same handler behind a QUIC
//! socket, not a second implementation.
//!
//! # Transport matrix
//!
//! | Mode | Protocol | TLS |
//! | --- | --- | --- |
//! | [`http1`] | HTTP/1.1 | none |
//! | [`http1_tls`] | HTTP/1.1 | TLS 1.3 |
//! | [`http3`] | HTTP/3 | TLS 1.3, always |
//!
//! There is no plaintext HTTP/3 row and there cannot be: QUIC embeds TLS 1.3
//! in the transport handshake, so an unencrypted HTTP/3 connection is not a
//! thing the protocol can express.
//!
//! [`Gateway::serve`]: crate::handler::Gateway::serve
//! [`Reply`]: crate::handler::Reply

pub mod cert;
pub mod http1;
pub mod tls;

#[cfg(feature = "http3")]
pub mod http3;

use crate::handler::{Gateway, Reply};
use bytes::Bytes;
use http::{Method, Request, Response};
use http_body_util::{BodyExt, Full};

/// Adapts an `http::Request` to the gateway and back.
///
/// Every listener funnels through this, which is what keeps their behaviour
/// identical: an HTTP/1.1 client and an HTTP/3 client hitting the same path get
/// byte-identical responses because they run the same code.
pub async fn handle<B>(gateway: &Gateway, request: Request<B>) -> Response<Full<Bytes>>
where
    B: http_body::Body,
    B::Error: std::fmt::Debug,
{
    let (parts, body) = request.into_parts();
    let uri = parts
        .uri
        .path_and_query()
        .map_or_else(|| parts.uri.path().to_string(), ToString::to_string);

    let bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes().to_vec(),
        // A body that failed mid-read is a malformed request, not a service
        // error, so it is reported as one rather than as a 500.
        Err(_) => return bad_body(),
    };

    let accept = parts
        .headers
        .get(http::header::ACCEPT)
        .and_then(|value| value.to_str().ok());

    into_response(gateway.serve_with(&parts.method, &uri, bytes, accept))
}

/// Converts a [`Reply`] into an `http::Response`.
fn into_response(reply: Reply) -> Response<Full<Bytes>> {
    let mut response = Response::builder().status(reply.status);
    if let Some(headers) = response.headers_mut() {
        headers.extend(reply.headers);
    }
    response
        .body(Full::new(Bytes::from(reply.body)))
        .unwrap_or_else(|_| {
            let mut fallback = Response::new(Full::new(Bytes::new()));
            *fallback.status_mut() = http::StatusCode::INTERNAL_SERVER_ERROR;
            fallback
        })
}

/// The response for a request body that could not be read.
fn bad_body() -> Response<Full<Bytes>> {
    let body = br#"{"error":{"code":400,"message":"Could not read the request body.","status":"INVALID_ARGUMENT"}}"#;
    Response::builder()
        .status(http::StatusCode::BAD_REQUEST)
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from_static(body)))
        .unwrap_or_else(|_| Response::new(Full::new(Bytes::new())))
}

/// The HTTP methods the catalog answers, for an `Allow` header on `OPTIONS`.
pub const ALLOWED: &[Method] = &[
    Method::GET,
    Method::POST,
    Method::PATCH,
    Method::DELETE,
    Method::OPTIONS,
];
