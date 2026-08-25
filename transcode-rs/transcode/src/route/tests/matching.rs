//! Matching, capture slicing, and percent-decoding.

use super::fixtures::*;
use crate::route::{DecodeError, decode_segment};

#[test]
fn matches_multi_segment_capture() {
    let r = route("GET", SHELVES_BOOKS, "", CAP_NAME_1_5, "", 0);
    assert_eq!(
        matched(&r, "/v1/shelves/s1/books/b1", ""),
        Some(vec![("name", "shelves/s1/books/b1".to_string())])
    );
    assert_eq!(matched(&r, "/v1/shelves/s1/books", ""), None);
    assert_eq!(matched(&r, "/v1/shelves/s1/books/b1/pages/p1", ""), None);
}

#[test]
fn matches_capture_followed_by_literal() {
    let r = route("GET", PARENT_BOOKS, "", CAP_PARENT_1_3, "", 0);
    assert_eq!(
        matched(&r, "/v1/shelves/s1/books", ""),
        Some(vec![("parent", "shelves/s1".to_string())])
    );
}

#[test]
fn multi_matches_zero_or_more() {
    let r = route("GET", V1_MULTI, "", CAP_NAME_TO_END, "", 0);
    assert_eq!(matched(&r, "/v1", ""), Some(vec![("name", String::new())]));
    assert_eq!(
        matched(&r, "/v1/a/b/c", ""),
        Some(vec![("name", "a/b/c".to_string())])
    );
}

#[test]
fn verb_is_not_part_of_the_capture() {
    // matchit accepts `/v1/{name}:cancel` as an ordinary route and folds
    // ":cancel" into `name`. The verb is its own thing here.
    let r = route("POST", V1_SINGLE, "cancel", CAP_NAME_1_2, "", 0);
    assert_eq!(
        matched(&r, "/v1/op1", "cancel"),
        Some(vec![("name", "op1".to_string())])
    );
    assert_eq!(matched(&r, "/v1/op1", ""), None);

    let verbless = route("GET", V1_SINGLE, "", CAP_NAME_1_2, "", 0);
    assert_eq!(matched(&verbless, "/v1/op1", "cancel"), None);
}

#[test]
fn single_does_not_match_an_empty_segment() {
    let r = route("GET", V1_SINGLE, "", CAP_NAME_1_2, "", 0);
    assert_eq!(matched(&r, "/v1/", ""), None);
}

#[test]
fn percent_2f_stays_encoded_inside_a_capture() {
    // README §1.2: decoding %2F would make this indistinguishable from the
    // genuinely three-segment "shelves/a/b".
    let r = route("GET", SHELVES_ONE, "", CAP_NAME_1_3, "", 0);
    assert_eq!(
        matched(&r, "/v1/shelves/a%2Fb", ""),
        Some(vec![("name", "shelves/a%2Fb".to_string())])
    );
    // Three segments; this route pins two.
    assert_eq!(matched(&r, "/v1/shelves/a/b", ""), None);
}

#[test]
fn other_escapes_decode() {
    assert_eq!(decode_segment("plain").unwrap(), "plain");
    assert_eq!(decode_segment("a%20b").unwrap(), "a b");
    assert_eq!(decode_segment("a%3Ab").unwrap(), "a:b");
    // Multi-byte UTF-8 is encoded per byte and must reassemble.
    assert_eq!(decode_segment("caf%C3%A9").unwrap(), "café");
    // Lowercase %2f is preserved as well as uppercase.
    assert_eq!(decode_segment("a%2fb").unwrap(), "a%2fb");
}

#[test]
fn malformed_encoding_is_rejected() {
    assert_eq!(decode_segment("a%2").unwrap_err(), DecodeError::Truncated);
    assert_eq!(decode_segment("a%").unwrap_err(), DecodeError::Truncated);
    assert_eq!(decode_segment("a%zz").unwrap_err(), DecodeError::BadHex);
    assert_eq!(decode_segment("a%FF").unwrap_err(), DecodeError::NotUtf8);
}

#[test]
fn decode_borrows_when_there_is_nothing_to_decode() {
    assert!(matches!(
        decode_segment("shelves"),
        Ok(std::borrow::Cow::Borrowed(_))
    ));
}
