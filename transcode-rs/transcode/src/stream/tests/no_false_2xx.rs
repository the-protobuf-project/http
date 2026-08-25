//! The rule itself.

use super::{denied, encode, exploded};
use crate::codec::Framing;
use crate::stream::{StreamState, StreamWriter, Termination};

#[test]
fn a_failure_before_the_first_message_keeps_its_real_status() {
    // The overwhelming majority of real failures — authorization, validation,
    // quota, not-found — happen here, and all of them get an honest status.
    let mut writer = StreamWriter::new(Framing::JsonArray, "application/json");
    assert_eq!(writer.state(), StreamState::Pending);

    match writer.fail(denied(), encode) {
        Termination::Deferred { error } => {
            assert_eq!(error.http.as_u16(), 403);
            assert_eq!(error.to_json()["error"]["code"], serde_json::json!(403));
        }
        other => panic!("expected Deferred, got {other:?}"),
    }
}

#[test]
fn a_deferred_failure_requires_no_truncation() {
    let mut writer = StreamWriter::new(Framing::JsonArray, "application/json");
    let termination = writer.fail(denied(), encode);

    // Nothing streaming-specific happened, so this is an ordinary error
    // response — no framing, no trailers, no truncation.
    assert!(!termination.requires_truncation());
    assert!(termination.trailers().is_none());
}

#[test]
fn a_failure_after_the_first_message_truncates() {
    // The status line is spent. Truncation is the only signal HTTP has left.
    let mut writer = StreamWriter::new(Framing::JsonArray, "application/json");
    writer.message(br#"{"name":"artists/miles"}"#);
    assert_eq!(writer.state(), StreamState::Committed);

    match writer.fail(exploded(), encode) {
        Termination::Truncate {
            frame,
            trailers,
            error,
        } => {
            let text = String::from_utf8(frame).unwrap();
            // The error goes out in-band first, so a client reading the body
            // learns why before the connection dies.
            assert!(text.starts_with(','), "{text}");
            assert!(text.contains(r#""code":500"#), "{text}");
            assert!(text.ends_with(']'), "{text}");

            assert_eq!(trailers.status, crate::error::Code::Internal as i32);
            // The operator's view stays complete even though the client's cannot.
            assert_eq!(error.http.as_u16(), 500);
        }
        other => panic!("expected Truncate, got {other:?}"),
    }
}

#[test]
fn truncation_is_demanded_explicitly() {
    // A transport that ignores this and closes cleanly reports success for a
    // failed RPC, which is precisely the grpc-gateway bug.
    let mut writer = StreamWriter::new(Framing::JsonArray, "application/json");
    writer.message(b"{}");
    assert!(writer.fail(exploded(), encode).requires_truncation());
}

#[test]
fn a_clean_finish_is_not_truncated() {
    let mut writer = StreamWriter::new(Framing::JsonArray, "application/json");
    writer.message(b"{}");

    match writer.finish() {
        Termination::Complete { close, trailers } => {
            assert_eq!(close, b"]");
            assert_eq!(trailers.status, 0);
        }
        other => panic!("expected Complete, got {other:?}"),
    }
}

#[test]
fn the_commit_boundary_is_exactly_the_first_message() {
    let mut writer = StreamWriter::new(Framing::JsonArray, "application/json");
    assert!(!writer.committed());
    writer.message(b"{}");
    assert!(writer.committed());
}

#[test]
fn every_framing_defers_a_pre_commit_failure() {
    // The rule is about the state machine, not the byte layout, so it must hold
    // identically for all four.
    for framing in [
        Framing::JsonArray,
        Framing::Sse,
        Framing::LineDelimited,
        Framing::LengthPrefixed,
    ] {
        let mut writer = StreamWriter::new(framing, "text/plain");
        assert!(
            matches!(writer.fail(denied(), encode), Termination::Deferred { .. }),
            "{framing:?}"
        );
    }
}

#[test]
fn every_framing_truncates_a_post_commit_failure() {
    for framing in [
        Framing::JsonArray,
        Framing::Sse,
        Framing::LineDelimited,
        Framing::LengthPrefixed,
    ] {
        let mut writer = StreamWriter::new(framing, "text/plain");
        writer.message(b"{}");
        assert!(
            writer.fail(exploded(), encode).requires_truncation(),
            "{framing:?}"
        );
    }
}
