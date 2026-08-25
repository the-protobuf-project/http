//! HTTP/3 over QUIC.
//!
//! Always encrypted: QUIC carries TLS 1.3 in its transport handshake, so there
//! is no plaintext mode to offer. The `alpn` for this listener is `h3`.
//!
//! What is worth noticing is how similar this is to the HTTP/1.1 listener. The
//! transport is entirely different — streams instead of a byte pipe, a QUIC
//! handshake instead of a TCP one — but the request handling is the same call
//! to the same function, because the handler never knew which transport it was
//! behind.

use super::handle;
use crate::handler::Handler;
use bytes::{Buf, Bytes};
use h3::server::Connection;
use http::Request;
use http_body_util::BodyExt;
use std::net::SocketAddr;
use std::sync::Arc;

/// Serves HTTP/3 over QUIC with TLS 1.3.
///
/// # Errors
///
/// Fails if the socket cannot be bound or the TLS configuration is rejected by
/// the QUIC layer. Per-connection failures are logged and the listener
/// continues.
pub async fn serve(
    handler: Handler,
    addr: SocketAddr,
    tls: rustls::ServerConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let quic_config = quinn::crypto::rustls::QuicServerConfig::try_from(tls)?;
    let server_config = quinn::ServerConfig::with_crypto(Arc::new(quic_config));
    let endpoint = quinn::Endpoint::server(server_config, addr)?;
    tracing::info!(%addr, "http/3 listening (quic, tls 1.3)");

    let handler = Arc::new(handler);
    while let Some(incoming) = endpoint.accept().await {
        let handler = Arc::clone(&handler);

        tokio::spawn(async move {
            match incoming.await {
                Ok(conn) => {
                    if let Err(err) = serve_connection(&handler, conn).await {
                        tracing::debug!(?err, "http/3 connection ended");
                    }
                }
                Err(err) => tracing::debug!(?err, "quic handshake failed"),
            }
        });
    }
    Ok(())
}

/// Serves every request on one QUIC connection.
async fn serve_connection(
    handler: &Arc<Handler>,
    conn: quinn::Connection,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut h3_conn = Connection::new(h3_quinn::Connection::new(conn)).await?;

    loop {
        match h3_conn.accept().await {
            Ok(Some(resolver)) => {
                let (request, stream) = resolver.resolve_request().await?;
                let handler = Arc::clone(handler);
                tokio::spawn(async move {
                    if let Err(err) = respond(&handler, request, stream).await {
                        tracing::debug!(?err, "http/3 request failed");
                    }
                });
            }
            // The peer closed the connection cleanly.
            Ok(None) => return Ok(()),
            Err(err) => return Err(err.into()),
        }
    }
}

/// Reads one request body, calls the handler, and writes the response.
async fn respond<S>(
    handler: &Handler,
    request: Request<()>,
    mut stream: h3::server::RequestStream<S, Bytes>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    S: h3::quic::BidiStream<Bytes> + Send + 'static,
{
    let mut body = Vec::new();
    while let Some(mut chunk) = stream.recv_data().await? {
        body.extend_from_slice(chunk.copy_to_bytes(chunk.remaining()).as_ref());
    }

    let (parts, ()) = request.into_parts();
    let rebuilt = Request::from_parts(parts, http_body_util::Full::new(Bytes::from(body)));

    let response = handle(handler, rebuilt).await;
    let (parts, response_body) = response.into_parts();

    stream
        .send_response(http::Response::from_parts(parts, ()))
        .await?;

    let bytes = response_body.collect().await?.to_bytes();
    if !bytes.is_empty() {
        stream.send_data(bytes).await?;
    }
    stream.finish().await?;
    Ok(())
}
