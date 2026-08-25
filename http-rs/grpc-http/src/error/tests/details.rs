//! Detail decoding, rendering, and redaction.

use super::DOMAIN;
use crate::error::{
    Code, DebugInfo, Detail, ErrorInfo, GatewayError, ResourceInfo, format_duration,
};
use prost::Message as _;
use serde_json::json;

#[test]
fn error_info_is_synthesised_when_the_service_supplies_none() {
    // AIP-193 requires an ErrorInfo on every error. A service returning a bare
    // status still gets one, or the caller cannot tell which service failed.
    let err = GatewayError::new(Code::NotFound, "gone").ensure_error_info(DOMAIN);
    let details = &err.to_json()["error"]["details"];

    assert_eq!(
        details[0]["@type"],
        json!("type.googleapis.com/google.rpc.ErrorInfo")
    );
    assert_eq!(details[0]["reason"], json!("NOT_FOUND"));
    assert_eq!(details[0]["domain"], json!(DOMAIN));
}

#[test]
fn a_supplied_error_info_is_not_duplicated() {
    let err = GatewayError::new(Code::NotFound, "gone")
        .with_error_info("RESOURCE_MISSING", DOMAIN, [])
        .ensure_error_info(DOMAIN);

    let count = err
        .details
        .iter()
        .filter(|d| matches!(d, Detail::ErrorInfo(_)))
        .count();
    assert_eq!(count, 1);
    assert_eq!(
        err.to_json()["error"]["details"][0]["reason"],
        json!("RESOURCE_MISSING")
    );
}

#[test]
fn debug_info_is_stripped() {
    let err = GatewayError::new(Code::Internal, "boom")
        .with_detail(Detail::DebugInfo(DebugInfo {
            stack_entries: vec!["frame one".into()],
            detail: "connection pool exhausted at db.rs:88".into(),
        }))
        .ensure_error_info(DOMAIN)
        .strip_debug_info();

    let rendered = err.to_json().to_string();
    assert!(!rendered.contains("db.rs"), "{rendered}");
    assert!(!rendered.contains("DebugInfo"), "{rendered}");
}

#[test]
fn details_decode_from_a_status_trailer() {
    let info = ErrorInfo {
        reason: "RESOURCE_MISSING".into(),
        domain: DOMAIN.into(),
        metadata: [("resource".to_string(), "shelves/s1/books/b9".to_string())]
            .into_iter()
            .collect(),
    };
    let any = prost_types::Any {
        type_url: "type.googleapis.com/google.rpc.ErrorInfo".into(),
        value: info.encode_to_vec(),
    };

    match Detail::from_any(&any) {
        Detail::ErrorInfo(got) => {
            assert_eq!(got.reason, "RESOURCE_MISSING");
            assert_eq!(got.metadata["resource"], "shelves/s1/books/b9");
        }
        other => panic!("expected an ErrorInfo, got {other:?}"),
    }
}

#[test]
fn an_unmodelled_detail_is_preserved_not_dropped() {
    // A service may attach its own detail types. Discarding them would make
    // them pointless, so they survive as Unknown with the payload intact.
    let any = prost_types::Any {
        type_url: "type.googleapis.com/library.v1.ShelfFull".into(),
        value: vec![8, 42],
    };
    match Detail::from_any(&any) {
        Detail::Unknown { type_url, value } => {
            assert_eq!(type_url, "type.googleapis.com/library.v1.ShelfFull");
            assert_eq!(value, vec![8, 42]);
        }
        other => panic!("expected Unknown, got {other:?}"),
    }
}

#[test]
fn a_corrupt_detail_degrades_to_unknown_rather_than_vanishing() {
    let any = prost_types::Any {
        type_url: "type.googleapis.com/google.rpc.BadRequest".into(),
        // Field 1 declared as a length-delimited message, length overruns.
        value: vec![0x0a, 0x7f, 0x01],
    };
    assert!(matches!(Detail::from_any(&any), Detail::Unknown { .. }));
}

#[test]
fn duration_renders_as_protojson() {
    let cases = [
        (0i64, 0i32, "0s"),
        (1, 0, "1s"),
        (1, 500_000_000, "1.500s"),
        (0, 340_012_000, "0.340012s"),
        (0, 1, "0.000000001s"),
        (-1, -500_000_000, "-1.500s"),
        // A mixed-sign representation is legal in the proto and must normalize.
        (1, -500_000_000, "0.500s"),
    ];
    for (seconds, nanos, want) in cases {
        let got = format_duration(&prost_types::Duration { seconds, nanos });
        assert_eq!(got, want, "{seconds}s {nanos}ns");
    }
}

#[test]
fn empty_fields_are_omitted_from_details() {
    // protojson omits defaults, and an error body full of `"owner": ""` is
    // noise a caller has to read past to find the real problem.
    let err = GatewayError::new(Code::NotFound, "gone")
        .with_detail(Detail::ResourceInfo(ResourceInfo {
            resource_type: "library.example.com/Book".into(),
            resource_name: "shelves/s1/books/b9".into(),
            owner: String::new(),
            description: String::new(),
        }))
        .ensure_error_info(DOMAIN);

    let rendered = err.to_json();
    let info = rendered["error"]["details"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["@type"] == json!("type.googleapis.com/google.rpc.ResourceInfo"))
        .unwrap();

    assert_eq!(info["resourceName"], json!("shelves/s1/books/b9"));
    assert!(info.get("owner").is_none());
    assert!(info.get("description").is_none());
}
