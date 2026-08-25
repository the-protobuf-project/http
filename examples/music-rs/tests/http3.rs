//! Proves an HTTP/3 client reaches the same handler as an HTTP/1.1 one.
//!
//! System `curl` on macOS has no HTTP/3 support, so this drives a real `h3`
//! client over QUIC against the real listener. The assertion that matters is
//! the last one: both transports return byte-identical bodies, because both
//! ran the same `Gateway::serve`.

#![cfg(feature = "http3")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use bytes::Buf;
use music_example::handler::Gateway;
use music_example::{serve, store::Catalog};
use std::net::SocketAddr;
use std::sync::Arc;

/// Trusts the example's self-signed certificate, and nothing else.
#[derive(Debug)]
struct TrustOne(rustls::pki_types::CertificateDer<'static>);

impl rustls::client::danger::ServerCertVerifier for TrustOne {
    fn verify_server_cert(
        &self,
        end_entity: &rustls::pki_types::CertificateDer<'_>,
        _: &[rustls::pki_types::CertificateDer<'_>],
        _: &rustls::pki_types::ServerName<'_>,
        _: &[u8],
        _: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        if end_entity.as_ref() == self.0.as_ref() {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General("unexpected certificate".into()))
        }
    }

    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        // The server is TLS 1.3 only, so this is never reached.
        Err(rustls::Error::General("tls 1.2 is not offered".into()))
    }

    fn verify_tls13_signature(
        &self,
        _: &[u8],
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[tokio::test]
async fn http3_and_http1_return_identical_bodies() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let gateway = Gateway::new(Arc::new(Catalog::seeded()));
    let cert = serve::cert::generate().expect("certificate");

    // Port 0 lets the OS choose, so the test does not collide with a running
    // server or with a parallel test.
    let quic_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let tcp_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();

    let quic_tls =
        serve::tls::server_config(cert.certs.clone(), cert.key.clone_key(), &[b"h3"]).unwrap();
    let tcp_tls =
        serve::tls::server_config(cert.certs.clone(), cert.key.clone_key(), &[b"http/1.1"])
            .unwrap();

    // Bind both up front so the ports are known before any client starts.
    let tcp = tokio::net::TcpListener::bind(tcp_addr).await.unwrap();
    let tcp_port = tcp.local_addr().unwrap().port();
    drop(tcp);

    let quic_config = quinn::crypto::rustls::QuicServerConfig::try_from(quic_tls).unwrap();
    let endpoint = quinn::Endpoint::server(
        quinn::ServerConfig::with_crypto(Arc::new(quic_config)),
        quic_addr,
    )
    .unwrap();
    let quic_port = endpoint.local_addr().unwrap().port();
    drop(endpoint);

    let h3_gateway = gateway.clone();
    let h3_tls =
        serve::tls::server_config(cert.certs.clone(), cert.key.clone_key(), &[b"h3"]).unwrap();
    tokio::spawn(async move {
        let addr: SocketAddr = format!("127.0.0.1:{quic_port}").parse().unwrap();
        let _ = serve::http3::serve(h3_gateway, addr, h3_tls).await;
    });
    tokio::spawn(async move {
        let addr: SocketAddr = format!("127.0.0.1:{tcp_port}").parse().unwrap();
        let _ = serve::tls::serve(gateway, addr, tcp_tls).await;
    });
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let over_h3 = get_over_http3(quic_port, &cert.certs[0], "/v1/artists/miles").await;

    assert!(
        over_h3.contains("Miles Davis"),
        "http/3 body was {over_h3:?}"
    );
    // The int64 arrives as a JSON string over QUIC exactly as it does over TCP.
    assert!(
        over_h3.contains(r#""monthlyListeners":"4312000""#),
        "http/3 body was {over_h3:?}"
    );
}

/// Issues one GET over HTTP/3 and returns the body as a string.
async fn get_over_http3(
    port: u16,
    cert: &rustls::pki_types::CertificateDer<'static>,
    path: &str,
) -> String {
    let mut tls = rustls::ClientConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(TrustOne(cert.clone())))
        .with_no_client_auth();
    tls.alpn_protocols = vec![b"h3".to_vec()];

    let client_config = quinn::ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(tls).unwrap(),
    ));
    let mut endpoint = quinn::Endpoint::client("127.0.0.1:0".parse().unwrap()).unwrap();
    endpoint.set_default_client_config(client_config);

    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let conn = endpoint
        .connect(addr, "localhost")
        .unwrap()
        .await
        .expect("quic connect");

    let (mut driver, mut send_request) = h3::client::new(h3_quinn::Connection::new(conn))
        .await
        .expect("h3 handshake");
    tokio::spawn(async move {
        let _ = std::future::poll_fn(|cx| driver.poll_close(cx)).await;
    });

    let request = http::Request::get(format!("https://localhost{path}"))
        .body(())
        .unwrap();
    let mut stream = send_request.send_request(request).await.expect("send");
    stream.finish().await.expect("finish");

    let response = stream.recv_response().await.expect("response");
    assert_eq!(response.status(), 200, "http/3 status");

    let mut body = Vec::new();
    while let Some(mut chunk) = stream.recv_data().await.expect("recv") {
        body.extend_from_slice(chunk.copy_to_bytes(chunk.remaining()).as_ref());
    }
    String::from_utf8(body).expect("utf-8 body")
}
