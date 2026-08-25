//! HTTP/1.1, plaintext and over TLS.

use super::handle;
use crate::handler::Gateway;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

/// Serves HTTP/1.1 in the clear.
///
/// For local development and for a deployment terminating TLS at a proxy. In
/// front of the open internet, use [`super::tls`].
///
/// # Errors
///
/// Fails if the address cannot be bound. Per-connection errors are logged and
/// the listener continues: one client hanging up mid-request is not a reason to
/// stop serving everyone else.
pub async fn serve(gateway: Gateway, addr: SocketAddr) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "http/1.1 listening (plaintext)");

    let gateway = Arc::new(gateway);
    loop {
        let (stream, peer) = listener.accept().await?;
        let gateway = Arc::clone(&gateway);

        tokio::spawn(async move {
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
