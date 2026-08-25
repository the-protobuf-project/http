//! `Accept` header parsing and preference ordering.

use crate::codec::{MediaType, parse_accept};

#[test]
fn quality_orders_entries() {
    let entries = parse_accept("application/json;q=0.5, text/event-stream;q=0.9");
    assert_eq!(entries[0].media.to_string(), "text/event-stream");
    assert_eq!(entries[0].quality, 900);
    assert_eq!(entries[1].quality, 500);
}

#[test]
fn absent_quality_is_full() {
    let entries = parse_accept("application/json");
    assert_eq!(entries[0].quality, 1000);
}

#[test]
fn specificity_breaks_a_quality_tie() {
    // RFC 9110: at equal quality the more specific range wins, so JSON is
    // preferred even though the wildcard was sent first.
    let entries = parse_accept("*/*, application/json");
    assert_eq!(entries[0].media.to_string(), "application/json");
    assert_eq!(entries[1].media.to_string(), "*/*");
}

#[test]
fn header_order_survives_a_full_tie() {
    let entries = parse_accept("application/json, application/x-protobuf");
    assert_eq!(entries[0].media.to_string(), "application/json");
    assert_eq!(entries[1].media.to_string(), "application/x-protobuf");
}

#[test]
fn q_values_are_exact_thousandths() {
    // Parsed by hand rather than via f32, so these are exact.
    assert_eq!(parse_accept("a/b;q=0.001")[0].quality, 1);
    assert_eq!(parse_accept("a/b;q=0.9")[0].quality, 900);
    assert_eq!(parse_accept("a/b;q=0.75")[0].quality, 750);
    assert_eq!(parse_accept("a/b;q=1.0")[0].quality, 1000);
    assert_eq!(parse_accept("a/b;q=0")[0].quality, 0);
}

#[test]
fn zero_quality_is_a_refusal() {
    let entries = parse_accept("application/json;q=0");
    assert!(entries[0].is_refusal());
}

#[test]
fn a_malformed_quality_degrades_to_acceptable() {
    // The safer direction: an ambiguous header should not silently refuse a
    // codec the client probably wanted.
    assert_eq!(parse_accept("a/b;q=abc")[0].quality, 1000);
}

#[test]
fn malformed_entries_are_skipped_not_fatal() {
    let entries = parse_accept("not-a-media-type, application/json");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].media.to_string(), "application/json");
}

#[test]
fn parameters_are_ignored_for_matching() {
    let media = MediaType::parse("application/json; charset=utf-8").unwrap();
    assert_eq!(media.to_string(), "application/json");
    assert!(media.matches("application/json"));
}

#[test]
fn wildcards_match_on_the_accept_side_only() {
    assert!(MediaType::parse("*/*").unwrap().matches("application/json"));
    assert!(
        MediaType::parse("application/*")
            .unwrap()
            .matches("application/json")
    );
    assert!(
        !MediaType::parse("text/*")
            .unwrap()
            .matches("application/json")
    );
    // A registered codec's own type is never a wildcard, so matching is not
    // symmetric: this is the request asking, not the codec offering.
    assert!(!MediaType::parse("application/json").unwrap().matches("*/*"));
}

#[test]
fn matching_is_case_insensitive() {
    assert!(
        MediaType::parse("Application/JSON")
            .unwrap()
            .matches("application/json")
    );
}
