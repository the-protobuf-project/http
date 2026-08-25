//! Listeners.
//!
//! The point of this module is how little there is in it. [`Handler::serve`]
//! is a plain function from a method, a URI, and a body to a [`Reply`] — it
//! knows nothing about connections, TLS, or protocol versions. Each listener
//! below is a small adapter that reads a request off its own transport, calls
//! that one function, and writes the answer back.
//!
//! That is the concrete payoff of the `tower::Service` shape in
//! the README: an HTTP/3 handler is the same handler behind a QUIC
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
//! [`Handler::serve`]: crate::handler::Handler::serve
//! [`Reply`]: crate::handler::Reply

pub mod cert;
pub mod http1;
pub mod tls;
pub mod truncate;

#[cfg(feature = "http3")]
pub mod http3;

use crate::handler::{Handler, Reply};
use bytes::Bytes;
use http::{Method, Request, Response};
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use truncate::{TruncatedStream, Truncating};

/// The body every listener writes.
///
/// Boxed with an error type rather than a plain [`Full`] because a stream that
/// failed after committing its status must end *abnormally* — see README §6.2.
/// A body that can only succeed has no way to express that, and a listener
/// holding one has no choice but to close cleanly and report success for a
/// failed RPC.
pub type ReplyBody = BoxBody<Bytes, TruncatedStream>;

/// Adapts an `http::Request` to the handler and back.
///
/// Every listener funnels through this, which is what keeps their behaviour
/// identical: an HTTP/1.1 client and an HTTP/3 client hitting the same path get
/// byte-identical responses because they run the same code.
pub async fn handle<B>(handler: &Handler, request: Request<B>) -> Response<ReplyBody>
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

    // Read alongside Accept and passed separately: they answer different
    // questions — what the body is, and what the caller will take — and
    // negotiation owes a different status to each (415 against 406).
    let content_type = parts
        .headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok());

    into_response(handler.serve_negotiated(&parts.method, &uri, bytes, accept, content_type))
}

/// The header a handler marks a truncated stream with.
///
/// Internal to the example: it carries the [`Termination::Truncate`] decision
/// from the handler out to the listener, and is stripped before the response is
/// written. A real integration would return the termination itself rather than
/// smuggling it through a header.
///
/// [`Termination::Truncate`]: transcode::stream::Termination::Truncate
const TRUNCATE_MARKER: &str = "x-handler-truncate";

/// Converts a [`Reply`] into an `http::Response`.
///
/// A reply carrying [`TRUNCATE_MARKER`] gets a body that yields its bytes and
/// then fails, so the listener ends the response abnormally instead of
/// completing it. Without this the example would write an error frame, set the
/// trailers, close cleanly — and report success for a stream that failed, which
/// is precisely the grpc-gateway behaviour this project exists to correct.
fn into_response(mut reply: Reply) -> Response<ReplyBody> {
    let truncate = reply.headers.remove(TRUNCATE_MARKER).is_some();

    let mut response = Response::builder().status(reply.status);
    if let Some(headers) = response.headers_mut() {
        headers.extend(reply.headers);
    }

    let bytes = Bytes::from(reply.body);
    let body = if truncate {
        Truncating::new(bytes).boxed()
    } else {
        Full::new(bytes).map_err(|never| match never {}).boxed()
    };

    response.body(body).unwrap_or_else(|_| {
        let mut fallback = Response::new(empty_body());
        *fallback.status_mut() = http::StatusCode::INTERNAL_SERVER_ERROR;
        fallback
    })
}

/// An empty body of the listener's body type.
fn empty_body() -> ReplyBody {
    Full::new(Bytes::new())
        .map_err(|never| match never {})
        .boxed()
}

/// The response for a request body that could not be read.
fn bad_body() -> Response<ReplyBody> {
    let body = br#"{"error":{"code":400,"message":"Could not read the request body.","status":"INVALID_ARGUMENT"}}"#;
    Response::builder()
        .status(http::StatusCode::BAD_REQUEST)
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(
            Full::new(Bytes::from_static(body))
                .map_err(|never| match never {})
                .boxed(),
        )
        .unwrap_or_else(|_| Response::new(empty_body()))
}

/// The HTTP methods the catalog answers, for an `Allow` header on `OPTIONS`.
pub const ALLOWED: &[Method] = &[
    Method::GET,
    Method::POST,
    Method::PATCH,
    Method::DELETE,
    Method::OPTIONS,
];
