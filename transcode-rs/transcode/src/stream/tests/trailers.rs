//! Trailer construction.

use super::{DOMAIN, encode, exploded};
use crate::codec::Framing;
use crate::error::{Code, Error};
use crate::stream::{StreamWriter, Termination, TrailerSet};

#[test]
fn a_clean_stream_reports_status_zero() {
    let trailers = TrailerSet::ok();
    assert_eq!(trailers.status, 0);

    let headers = trailers.to_headers();
    assert_eq!(headers.get("grpc-status").unwrap(), "0");
    // No message on success; an empty one would be noise.
    assert!(headers.get("grpc-message").is_none());
}

#[test]
fn a_failure_reports_the_canonical_code_as_a_number() {
    // Trailers carry the gRPC code, unlike the AIP-193 envelope's `code`, which
    // is the HTTP status. The two are different fields answering different
    // questions, and conflating them is the grpc-gateway bug.
    let err = Error::new(Code::NotFound, "gone").ensure_error_info(DOMAIN);
    let trailers = TrailerSet::from_error(&err);

    assert_eq!(trailers.status, Code::NotFound as i32);
    assert_eq!(trailers.to_headers().get("grpc-status").unwrap(), "5");
}

#[test]
fn grpc_message_is_percent_encoded() {
    // A status message routinely holds a resource name or a quoted value, and a
    // raw newline in a header value is a smuggling vector, not a formatting nit.
    let err = Error::new(Code::Internal, "line one\nline two");
    let trailers = TrailerSet::from_error(&err);

    assert!(!trailers.message.contains('\n'), "{}", trailers.message);
    assert!(trailers.message.contains("%0A"), "{}", trailers.message);
}

#[test]
fn non_ascii_messages_survive_as_escapes() {
    let err = Error::new(Code::NotFound, "café introuvable");
    let trailers = TrailerSet::from_error(&err);

    assert!(trailers.message.is_ascii(), "{}", trailers.message);
    // The two UTF-8 bytes of "é".
    assert!(trailers.message.contains("%C3%A9"), "{}", trailers.message);
}

#[test]
fn trailers_are_advertised_in_the_response_headers() {
    // An intermediary that has not been told to expect trailers may drop them,
    // so the announcement has to be in the headers, which go out first.
    let writer = StreamWriter::new(Framing::JsonArray, "application/json");
    let headers = writer.headers();

    assert_eq!(
        headers.get(http::header::TRAILER).unwrap(),
        "grpc-status, grpc-message"
    );
}

#[test]
fn a_stream_is_never_cached() {
    // A cached replay would serve a prefix of a stream as if it were whole.
    let writer = StreamWriter::new(Framing::Sse, "text/event-stream");
    assert_eq!(
        writer.headers().get(http::header::CACHE_CONTROL).unwrap(),
        "no-cache"
    );
}

#[test]
fn a_truncated_stream_still_carries_trailers() {
    // Belt and braces: trailers only reach a client that asked for them, so
    // they are never the only signal — but they are still sent.
    let mut writer = StreamWriter::new(Framing::JsonArray, "application/json");
    writer.message(b"{}");

    match writer.fail(exploded(), encode) {
        Termination::Truncate { trailers, .. } => {
            assert_eq!(trailers.status, Code::Internal as i32);
            assert!(trailers.to_headers().contains_key("grpc-message"));
        }
        other => panic!("expected Truncate, got {other:?}"),
    }
}
