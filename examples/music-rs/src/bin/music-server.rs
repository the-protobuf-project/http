//! Runs the music catalog over every transport at once.
//!
//! ```text
//! cargo run -p music-example --features http3 --bin music-server
//!
//!   http://127.0.0.1:8080   HTTP/1.1, plaintext
//!   https://127.0.0.1:8443  HTTP/1.1, TLS 1.3
//!   https://127.0.0.1:8443  HTTP/3 over QUIC, TLS 1.3   (UDP)
//! ```
//!
//! `MUSIC_HTTP_ADDR` and `MUSIC_TLS_ADDR` override the listeners. The
//! conformance script sets them so this server and the Go one can run at once
//! and be asked the same questions — which is the only way to check that two
//! runtimes generated from one IR actually answer alike.
//!
//! The TLS listeners share a self-signed certificate generated at startup, so
//! a client needs `--insecure` or the printed PEM to connect.
//!
//! All three serve the same [`Handler`] value. That is the demonstration: the
//! handler is written once and is unaware of which transport reached it.

use music_example::handler::Handler;
use music_example::serve;
use music_example::store::Catalog;
use std::net::SocketAddr;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // rustls needs a process-wide crypto provider before any config is built.
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| "failed to install the rustls crypto provider")?;

    let catalog = Arc::new(Catalog::seeded());
    let handler = Handler::new(catalog);

    let plain: SocketAddr = addr_from_env("MUSIC_HTTP_ADDR", "127.0.0.1:8080")?;
    let secure: SocketAddr = addr_from_env("MUSIC_TLS_ADDR", "127.0.0.1:8443")?;

    let cert = serve::cert::generate()?;
    println!("{}", banner(&cert.cert_pem, plain, secure));

    // ALPN differs per transport: h3 for QUIC, http/1.1 for TCP. A client that
    // offers neither is refused during the handshake.
    let tcp_tls =
        serve::tls::server_config(cert.certs.clone(), cert.key.clone_key(), &[b"http/1.1"])?;
    let quic_tls = serve::tls::server_config(cert.certs, cert.key, &[b"h3"])?;

    let mut tasks = tokio::task::JoinSet::new();
    tasks.spawn(serve::http1::serve(handler.clone(), plain));
    {
        let handler = handler.clone();
        tasks.spawn(async move { serve::tls::serve(handler, secure, tcp_tls).await });
    }
    #[cfg(feature = "http3")]
    {
        let handler = handler.clone();
        tokio::spawn(async move {
            if let Err(err) = serve::http3::serve(handler, secure, quic_tls).await {
                tracing::error!(?err, "http/3 listener stopped");
            }
        });
    }
    #[cfg(not(feature = "http3"))]
    let _ = quic_tls;

    tokio::signal::ctrl_c().await?;
    println!("\nshutting down");
    tasks.shutdown().await;
    Ok(())
}

/// Reads a listen address from the environment, falling back to a default.
///
/// A malformed value is an error rather than a silent fallback: a server that
/// quietly ignores the address it was told to use is one that looks up and
/// finds the wrong process listening.
fn addr_from_env(
    key: &str,
    fallback: &str,
) -> Result<SocketAddr, Box<dyn std::error::Error + Send + Sync>> {
    let raw = std::env::var(key).unwrap_or_else(|_| fallback.to_string());
    raw.parse()
        .map_err(|err| format!("{key}={raw:?} is not a socket address: {err}").into())
}

/// The startup banner, listing what is reachable where.
fn banner(cert_pem: &str, plain: SocketAddr, secure: SocketAddr) -> String {
    let http3 = if cfg!(feature = "http3") {
        format!("  https://{secure}  HTTP/3 over QUIC, TLS 1.3 (UDP)\n")
    } else {
        "  (HTTP/3 not compiled in; build with --features http3)\n".to_string()
    };
    format!(
        "music catalog\n\
         \n  http://{plain}   HTTP/1.1, plaintext\n\
           https://{secure}  HTTP/1.1, TLS 1.3\n\
         {http3}\n\
         try:\n  \
           curl http://{plain}/v1/artists\n  \
           curl -k https://{secure}/v1/artists/miles\n\
         \nself-signed certificate:\n{cert_pem}"
    )
}
