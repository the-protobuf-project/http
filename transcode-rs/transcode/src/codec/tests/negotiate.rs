//! Content negotiation, per README §3

use super::fixtures::{DOMAIN, JSON, NDJSON, PROTO, SSE, registry};
use crate::codec::{Negotiation, request_codec, response_codec};

/// Negotiates a response for a unary method.
fn unary(alt: Option<&str>, accept: Option<&str>) -> Result<usize, u16> {
    negotiate(alt, accept, None, false)
}

/// Negotiates a response, returning the codec index or the HTTP status.
fn negotiate(
    alt: Option<&str>,
    accept: Option<&str>,
    content_type: Option<&str>,
    streaming: bool,
) -> Result<usize, u16> {
    let reg = registry();
    let negotiation = Negotiation {
        content_type,
        accept,
        alt,
        streaming,
    };
    let request = request_codec(&reg, &negotiation, DOMAIN).map_err(|e| e.http.as_u16())?;
    response_codec(&reg, &negotiation, request, DOMAIN)
        .map(|e| e.index)
        .map_err(|e| e.http.as_u16())
}

#[test]
fn no_preference_yields_the_default() {
    assert_eq!(unary(None, None), Ok(JSON));
}

#[test]
fn alt_outranks_accept() {
    // An explicit ?alt= is the client being unambiguous; Accept is a preference.
    assert_eq!(unary(Some("proto"), Some("application/json")), Ok(PROTO));
}

#[test]
fn accept_selects_when_no_alt_is_given() {
    assert_eq!(unary(None, Some("application/x-protobuf")), Ok(PROTO));
}

#[test]
fn accept_honours_quality_order() {
    assert_eq!(
        unary(
            None,
            Some("application/json;q=0.2, application/x-protobuf;q=0.8")
        ),
        Ok(PROTO)
    );
}

#[test]
fn accept_wildcard_yields_the_default() {
    assert_eq!(unary(None, Some("*/*")), Ok(JSON));
}

#[test]
fn a_refused_codec_is_not_selected() {
    // q=0 is a refusal, not a low preference. JSON is excluded outright, so
    // the protobuf entry wins despite being second.
    assert_eq!(
        unary(None, Some("application/json;q=0, application/x-protobuf")),
        Ok(PROTO)
    );
}

#[test]
fn the_request_codec_is_the_fallback() {
    // No ?alt= and no Accept: answer in whatever the client sent.
    assert_eq!(
        negotiate(None, None, Some("application/x-protobuf"), false),
        Ok(PROTO)
    );
}

#[test]
fn content_type_parameters_are_ignored() {
    assert_eq!(
        negotiate(None, None, Some("application/json; charset=utf-8"), false),
        Ok(JSON)
    );
}

#[test]
fn an_unregistered_content_type_is_415() {
    assert_eq!(
        negotiate(None, None, Some("application/xml"), false),
        Err(415)
    );
}

#[test]
fn an_unsatisfiable_accept_is_406() {
    // The handler does not fall back to a codec the client excluded: answering
    // in a refused media type is worse than reporting no overlap.
    assert_eq!(unary(None, Some("application/xml")), Err(406));
}

#[test]
fn an_unknown_alt_is_400() {
    assert_eq!(unary(Some("yaml"), None), Err(400));
}

#[test]
fn sse_is_rejected_on_a_unary_method() {
    // A one-event stream is a worse answer than a plain body, so asking for it
    // explicitly is an error rather than something to silently reinterpret.
    assert_eq!(unary(Some("sse"), None), Err(400));
}

#[test]
fn sse_is_selectable_on_a_streaming_method() {
    assert_eq!(negotiate(Some("sse"), None, None, true), Ok(SSE));
}

#[test]
fn accept_event_stream_selects_sse_without_alt() {
    assert_eq!(
        negotiate(None, Some("text/event-stream"), None, true),
        Ok(SSE)
    );
}

#[test]
fn a_streaming_only_codec_is_skipped_when_accept_offers_an_alternative() {
    // The client will take either; only one of the two is legal here.
    assert_eq!(
        unary(None, Some("text/event-stream, application/json")),
        Ok(JSON)
    );
}

#[test]
fn a_wildcard_accept_never_selects_a_streaming_only_codec_for_unary() {
    // `*/*` must not resolve to SSE on a unary method just because SSE is
    // registered. It falls through to the default instead.
    assert_eq!(unary(None, Some("*/*")), Ok(JSON));
}

#[test]
fn ndjson_is_available_for_grpc_gateway_compatible_clients() {
    assert_eq!(
        negotiate(None, Some("application/x-ndjson"), None, true),
        Ok(NDJSON)
    );
}

#[test]
fn a_body_less_request_needs_no_request_codec() {
    let reg = registry();
    let negotiation = Negotiation::default();
    assert!(request_codec(&reg, &negotiation, DOMAIN).unwrap().is_none());
}
