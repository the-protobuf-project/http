//! Content negotiation, per README §3

use super::accept::parse_accept;
use super::registry::{CodecEntry, CodecRegistry};
use crate::error::{GatewayError, Result};

/// What the request asked for, gathered from the parts that select a codec.
///
/// Borrowed from the request, so negotiating allocates only when it fails.
#[derive(Debug, Clone, Copy, Default)]
pub struct Negotiation<'a> {
    /// The request's `Content-Type`, if it carried a body.
    pub content_type: Option<&'a str>,

    /// The request's `Accept` header, if present.
    pub accept: Option<&'a str>,

    /// The `?alt=` query parameter, if present.
    pub alt: Option<&'a str>,

    /// Whether the method streams its response, which decides whether a
    /// streaming-only codec such as SSE is a legal choice.
    pub streaming: bool,
}

/// Selects the codec that decodes the request body.
///
/// # Errors
///
/// Returns `415 Unsupported Media Type` when the `Content-Type` names no
/// registered codec. A request with no body needs no codec and yields `None`.
pub fn request_codec(
    registry: &CodecRegistry,
    negotiation: &Negotiation<'_>,
    domain: &str,
) -> Result<Option<&'static CodecEntry>> {
    let Some(content_type) = negotiation.content_type else {
        return Ok(None);
    };
    registry
        .by_media_type(content_type)
        .map(Some)
        .ok_or_else(|| {
            Box::new(GatewayError::unsupported_media_type(
                content_type,
                &registry.supported_media_types(),
                domain,
            ))
        })
}

/// Selects the codec that encodes the response.
///
/// The order is fixed by README §3: an explicit `?alt=` wins, then
/// `Accept`, then whatever decoded the request, then the registry default.
///
/// # Errors
///
/// - `400 INVALID_ARGUMENT` when `?alt=` names an unknown codec, or names a
///   streaming-only codec on a unary method.
/// - `406 Not Acceptable` when `Accept` is present and nothing in it is
///   registered. The gateway does not fall back to a codec the client
///   excluded — answering in a media type they refused is worse than telling
///   them there is no overlap.
pub fn response_codec(
    registry: &CodecRegistry,
    negotiation: &Negotiation<'_>,
    request: Option<&'static CodecEntry>,
    domain: &str,
) -> Result<&'static CodecEntry> {
    if let Some(alt) = negotiation.alt {
        return select_by_alt(registry, alt, negotiation.streaming, domain);
    }
    if let Some(accept) = negotiation.accept {
        if let Some(entry) = select_by_accept(registry, accept, negotiation.streaming) {
            return Ok(entry);
        }
        // A wildcard means "anything", so reaching here with one present means
        // the only matches were streaming-only codecs on a unary method.
        if !accepts_anything(accept) {
            return Err(Box::new(GatewayError::not_acceptable(
                accept,
                &registry.supported_media_types(),
                domain,
            )));
        }
    }

    let fallback = request.unwrap_or_else(|| registry.default_codec());
    if fallback.framing.allows_unary() || negotiation.streaming {
        return Ok(fallback);
    }
    Ok(registry.default_codec())
}

/// Resolves an explicit `?alt=` selection.
fn select_by_alt(
    registry: &CodecRegistry,
    alt: &str,
    streaming: bool,
    domain: &str,
) -> Result<&'static CodecEntry> {
    let entry = registry.by_name(alt).ok_or_else(|| {
        Box::new(
            GatewayError::new(
                crate::error::Code::InvalidArgument,
                format!("Unknown response format {alt:?}."),
            )
            .with_error_info(
                "UNKNOWN_RESPONSE_FORMAT",
                domain,
                [("supported".into(), registry.names().join(", "))],
            ),
        )
    })?;

    if !streaming && !entry.framing.allows_unary() {
        return Err(Box::new(
            GatewayError::new(
                crate::error::Code::InvalidArgument,
                format!("Response format {alt:?} is only available for streaming methods."),
            )
            .with_error_info(
                "STREAMING_ONLY_FORMAT",
                domain,
                [("format".into(), alt.to_string())],
            ),
        ));
    }
    Ok(entry)
}

/// Walks the `Accept` entries in preference order, returning the first
/// registered codec that is legal for this method.
fn select_by_accept(
    registry: &CodecRegistry,
    accept: &str,
    streaming: bool,
) -> Option<&'static CodecEntry> {
    for entry in parse_accept(accept) {
        if entry.is_refusal() {
            continue;
        }
        let found = registry.entries().iter().find(|codec| {
            (streaming || codec.framing.allows_unary())
                && codec
                    .media_types
                    .iter()
                    .any(|media| entry.media.matches(media))
        });
        if found.is_some() {
            return found;
        }
    }
    None
}

/// Whether an `Accept` header contains a non-refused `*/*`.
fn accepts_anything(accept: &str) -> bool {
    parse_accept(accept)
        .iter()
        .any(|e| !e.is_refusal() && e.media.is_any())
}
