//! Byte layout of the four framings.

use super::{encode, exploded};
use crate::codec::Framing;
use crate::stream::{FrameWriter, StreamWriter, Termination};

/// Streams two messages and returns the whole body, including the close.
fn body(framing: Framing, messages: &[&[u8]]) -> Vec<u8> {
    let mut writer = StreamWriter::new(framing, "text/plain");
    let mut out = Vec::new();
    for message in messages {
        out.extend_from_slice(&writer.message(message));
    }
    if let Termination::Complete { close, .. } = writer.finish() {
        out.extend_from_slice(&close);
    }
    out
}

#[test]
fn json_array_is_valid_json_when_complete() {
    let out = body(Framing::JsonArray, &[br#"{"n":1}"#, br#"{"n":2}"#]);
    let text = String::from_utf8(out).unwrap();

    assert_eq!(text, r#"[{"n":1},{"n":2}]"#);
    // The point of the framing: it parses as a whole at the end.
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 2);
}

#[test]
fn an_empty_json_array_stream_is_still_well_formed() {
    // Zero messages must not produce a bare `]` or an empty body.
    let out = body(Framing::JsonArray, &[]);
    assert_eq!(String::from_utf8(out).unwrap(), "[]");
}

#[test]
fn sse_emits_one_named_event_per_message() {
    let out = body(Framing::Sse, &[br#"{"n":1}"#, br#"{"n":2}"#]);
    let text = String::from_utf8(out).unwrap();

    assert_eq!(
        text,
        "event: message\ndata: {\"n\":1}\n\nevent: message\ndata: {\"n\":2}\n\n"
    );
}

#[test]
fn sse_offers_a_keepalive_and_the_others_do_not() {
    // An idle SSE connection is reaped by intermediaries, and a stream that is
    // merely waiting looks identical to one that died.
    let sse = StreamWriter::new(Framing::Sse, "text/event-stream");
    assert_eq!(sse.keepalive().unwrap(), b": keepalive\n\n");

    for framing in [
        Framing::JsonArray,
        Framing::LineDelimited,
        Framing::LengthPrefixed,
    ] {
        assert!(
            StreamWriter::new(framing, "text/plain")
                .keepalive()
                .is_none(),
            "{framing:?}"
        );
    }
}

#[test]
fn line_delimited_matches_what_grpc_gateway_emits() {
    let out = body(Framing::LineDelimited, &[br#"{"n":1}"#, br#"{"n":2}"#]);
    assert_eq!(String::from_utf8(out).unwrap(), "{\"n\":1}\n{\"n\":2}\n");
}

#[test]
fn length_prefixed_carries_an_explicit_length() {
    // Line-delimiting bytes that may contain a newline does not work, so a
    // binary framing has to say how long each message is.
    let out = body(Framing::LengthPrefixed, &[b"ab", b"cde"]);
    assert_eq!(out, [0, 0, 0, 2, b'a', b'b', 0, 0, 0, 3, b'c', b'd', b'e']);
}

#[test]
fn a_length_prefixed_message_may_contain_a_newline() {
    let out = body(Framing::LengthPrefixed, &[b"a\nb"]);
    assert_eq!(out, [0, 0, 0, 3, b'a', b'\n', b'b']);
}

#[test]
fn the_sse_error_frame_has_its_own_event_name() {
    // So a browser handler can bind to it rather than inspecting every message.
    let mut writer = StreamWriter::new(Framing::Sse, "text/event-stream");
    writer.message(br#"{"n":1}"#);

    match writer.fail(exploded(), encode) {
        Termination::Truncate { frame, .. } => {
            let text = String::from_utf8(frame).unwrap();
            assert!(text.starts_with("event: error\ndata: "), "{text}");
            assert!(text.contains(r#""code":500"#), "{text}");
        }
        other => panic!("expected Truncate, got {other:?}"),
    }
}

#[test]
fn a_json_array_error_closes_the_array_it_opened() {
    // Even a failed stream leaves parseable JSON behind, for a client that
    // reads the body rather than only the status.
    let mut frames = FrameWriter::new(Framing::JsonArray);
    let _ = frames.message(br#"{"n":1}"#);
    let error = frames.error(br#"{"error":{"code":500}}"#);

    let whole = format!("[{{\"n\":1}}{}", String::from_utf8(error).unwrap());
    let parsed: serde_json::Value = serde_json::from_str(&whole).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 2);
}
