//! TLS 1.3, and the HTTP/1.1 listener that terminates it.
//!
//! TLS 1.3 only. rustls supports 1.2, but this pins 1.3 because everything the
//! project targets speaks it, and because the version negotiation that makes
//! 1.2 reachable is where downgrade attacks live.

use super::handle;
use crate::handler::Gateway;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use rustls::ServerConfig;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

/// Builds a TLS 1.3 server configuration.
///
/// `alpn` advertises which protocols the listener speaks — `h2` and
/// `http/1.1` for TCP, `h3` for QUIC. A client that offers none of them is
/// refused during the handshake rather than after.
///
/// # Errors
///
/// Fails when the certificate and key do not form a usable pair.
pub fn server_config(
    certs: Vec<CertificateDer<'static>>,
    key: PrivateKeyDer<'static>,
    alpn: &[&[u8]],
) -> Result<ServerConfig, rustls::Error> {
    // `with_protocol_versions` rather than `with_safe_defaults`: the default
    // includes TLS 1.2, and this listener is 1.3 only.
    let mut config = ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
        .with_no_client_auth()
        .with_single_cert(certs, key)?;

    config.alpn_protocols = alpn.iter().map(|p| p.to_vec()).collect();
    Ok(config)
}

/// Serves HTTP/1.1 over TLS 1.3.
///
/// # Errors
///
/// Fails if the address cannot be bound. A failed handshake closes that one
/// connection and is logged; the listener keeps serving.
pub async fn serve(
    gateway: Gateway,
    addr: SocketAddr,
    config: ServerConfig,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    let acceptor = TlsAcceptor::from(Arc::new(config));
    tracing::info!(%addr, "http/1.1 listening (tls 1.3)");

    let gateway = Arc::new(gateway);
    loop {
        let (stream, peer) = listener.accept().await?;
        let acceptor = acceptor.clone();
        let gateway = Arc::clone(&gateway);

        tokio::spawn(async move {
            let stream = match acceptor.accept(stream).await {
                Ok(stream) => stream,
                Err(err) => {
                    tracing::debug!(%peer, ?err, "tls handshake failed");
                    return;
                }
            };

            let io = TokioIo::new(stream);
            let service = service_fn(move |req| {
                let gateway = Arc::clone(&gateway);
                async move { Ok::<_, std::convert::Infallible>(handle(&gateway, req).await) }
            });

            if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                tracing::debug!(%peer, ?err, "connection closed");
            }
        });
    }
}
