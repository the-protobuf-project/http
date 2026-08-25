//! Content negotiation for one request.
//!
//! The runtime decides this; the example only has to ask. Doing it here rather
//! than inside a method handler is what the generator will emit, because the
//! answer is a property of the request and the codec registry, not of the
//! method: a `415` is owed to a caller whose `Content-Type` names no codec even
//! when the method they aimed at does not read a body.

use crate::generated::{CODECS, DOMAIN};
use transcode::codec::{CodecEntry, CodecRegistry, Negotiation};
use transcode::error::Result;

/// The codecs this example was generated with.
pub(super) fn registry() -> CodecRegistry {
    CodecRegistry::new(CODECS)
}

/// The codecs one request decodes and encodes with.
///
/// Public because it is reachable through [`Call`](super::Call), which a
/// method handler receives.
#[derive(Debug, Clone, Copy)]
pub struct Codecs {
    /// The codec the request body decodes with, `None` when there is no body.
    ///
    /// A handler that reads a body checks this rather than assuming JSON: it is
    /// what a second codec would arrive through.
    pub request: Option<&'static CodecEntry>,

    /// The codec the response encodes with. Always resolved — negotiation ends
    /// at the registry default rather than at nothing.
    pub response: &'static CodecEntry,
}

/// Negotiates both codecs, per README §3.
///
/// # Errors
///
/// `415` when `Content-Type` names no registered codec, `406` when nothing in
/// `Accept` does, and `400` when `?alt=` names an unknown one. Each is decided
/// before any body is read, so a rejection costs nothing.
pub(super) fn negotiate(
    content_type: Option<&str>,
    accept: Option<&str>,
    alt: Option<&str>,
    streaming: bool,
) -> Result<Codecs> {
    let registry = registry();
    let negotiation = Negotiation {
        content_type,
        accept,
        alt,
        streaming,
    };

    let request = transcode::codec::request_codec(&registry, &negotiation, DOMAIN)?;
    let response = transcode::codec::response_codec(&registry, &negotiation, request, DOMAIN)?;
    Ok(Codecs { request, response })
}
