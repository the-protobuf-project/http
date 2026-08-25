//! A self-signed certificate for the example.
//!
//! Generated at startup rather than committed, so no private key lives in the
//! repository and every run gets a fresh one. A real deployment supplies its
//! own; this exists only so `cargo run` can demonstrate TLS without setup.

use rustls::pki_types::{CertificateDer, PrivateKeyDer};

/// A certificate and its key, valid for `localhost` and the loopback address.
pub struct SelfSigned {
    /// The certificate chain, one self-signed leaf.
    pub certs: Vec<CertificateDer<'static>>,
    /// The matching private key.
    pub key: PrivateKeyDer<'static>,
    /// The PEM encoding of the certificate, for a client that must trust it.
    pub cert_pem: String,
}

/// Generates a certificate for `localhost`, `127.0.0.1`, and `::1`.
///
/// # Errors
///
/// Fails only if key generation itself fails.
pub fn generate() -> Result<SelfSigned, rcgen::Error> {
    let subject_alt_names = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
    ];
    let certified = rcgen::generate_simple_self_signed(subject_alt_names)?;

    let cert_pem = certified.cert.pem();
    let key_der = PrivateKeyDer::try_from(certified.key_pair.serialize_der())
        .map_err(|_| rcgen::Error::CouldNotParseKeyPair)?;

    Ok(SelfSigned {
        certs: vec![certified.cert.der().clone()],
        key: key_der,
        cert_pem,
    })
}

impl std::fmt::Debug for SelfSigned {
    /// Deliberately prints no key material.
    ///
    /// `PrivateKeyDer` has no `Debug` of its own precisely so a key cannot be
    /// logged by accident, and this preserves that: the struct is inspectable
    /// without the secret ever reaching a log line.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SelfSigned")
            .field("certs", &self.certs.len())
            .field("key", &"<redacted>")
            .finish()
    }
}
