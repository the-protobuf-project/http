//! Listeners.
//!
//! The point of this module is how little there is in it. The gateway is a
//! `tower::Service` from an `http::Request` to an `http::Response` — it knows
//! nothing about connections, TLS, or protocol versions. Each listener below is
//! a small adapter that reads a request off its own transport, calls
//! [`handle`], and writes the answer back frame by frame.
//!
//! That is the concrete payoff of the `tower::Service` shape in the README: an
//! HTTP/3 handler is the same handler behind a QUIC socket, not a second
//! implementation.
//!
//! # Transport matrix
//!
//! | Mode | Protocol | TLS |
//! | --- | --- | --- |
//! | [`http1`] | HTTP/1.1 | none |
//! | [`tls`] | HTTP/1.1 | TLS 1.3 |
//! | `http3` | HTTP/3 | TLS 1.3, always |
//!
//! There is no plaintext HTTP/3 row and there cannot be: QUIC embeds TLS 1.3
//! in the transport handshake, so an unencrypted HTTP/3 connection is not a
//! thing the protocol can express.

pub mod cert;
pub mod http1;
pub mod tls;

#[cfg(feature = "http3")]
pub mod http3;

use crate::handler::Handler;
use http::{Request, Response};
use std::net::SocketAddr;
use transcode::gateway::GatewayBody;

/// Serves one request, recording the peer so interceptors such as
/// `RealIp::direct` can see it.
pub async fn handle<B>(
    handler: &Handler,
    peer: SocketAddr,
    mut request: Request<B>,
) -> Response<GatewayBody>
where
    B: http_body::Body + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    request.extensions_mut().insert(peer);
    handler.handle(request).await
}
