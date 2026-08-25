//! Table resolution: the three outcomes of README §1.5

use super::fixtures::*;
use crate::route::{DecodeError, Resolution, Route, RouteTable};

/// A table exercising every shape at once, in the order the generator would
/// emit it: most specific first.
static TABLE: &[Route] = &[
    route(
        "GET",
        SHELVES_BOOKS,
        "",
        CAP_NAME_1_5,
        "/v1/{name=shelves/*/books/*}",
        0,
    ),
    route(
        "DELETE",
        SHELVES_BOOKS,
        "",
        CAP_NAME_1_5,
        "/v1/{name=shelves/*/books/*}",
        1,
    ),
    route(
        "POST",
        V1_SINGLE,
        "cancel",
        CAP_NAME_1_2,
        "/v1/{name}:cancel",
        2,
    ),
    route("GET", V1_MULTI, "", CAP_NAME_TO_END, "/v1/{name=**}", 3),
];

#[test]
fn resolves_to_the_first_matching_route() {
    let t = RouteTable::new(TABLE);
    match t.resolve("GET", "/v1/shelves/s1/books/b1") {
        Resolution::Matched(m) => {
            assert_eq!(m.route.handler, 0);
            let captures = m.captures().unwrap();
            assert_eq!(captures.len(), 1);
            assert_eq!(captures[0].0, "name");
            assert_eq!(captures[0].1, "shelves/s1/books/b1");
        }
        other => panic!("expected a match, got {other:?}"),
    }
}

#[test]
fn method_mismatch_is_405_with_allow() {
    let t = RouteTable::new(TABLE);
    // PATCH is bound nowhere, but the path is. This must stay a 405: the bug in
    // grpc-gateway is that it routes this through UNIMPLEMENTED into a 501.
    match t.resolve("PATCH", "/v1/shelves/s1/books/b1") {
        Resolution::MethodNotAllowed { allow } => {
            assert!(allow.contains(&"GET"), "allow = {allow:?}");
            assert!(allow.contains(&"DELETE"), "allow = {allow:?}");
        }
        other => panic!("expected 405, got {other:?}"),
    }
}

#[test]
fn unknown_path_is_404() {
    static ONLY_LONG: &[Route] = &[route(
        "GET",
        SHELVES_BOOKS,
        "",
        CAP_NAME_1_5,
        "/v1/{name=shelves/*/books/*}",
        0,
    )];
    let t = RouteTable::new(ONLY_LONG);
    assert!(matches!(t.resolve("GET", "/v2/x"), Resolution::NotFound));
}

#[test]
fn a_colon_no_route_claims_is_data_not_a_verb() {
    // "/v1/a:b" with no matching verb route must bind name = "a:b" rather than
    // strip ":b". A ':' is legal inside a resource id.
    let t = RouteTable::new(TABLE);
    match t.resolve("GET", "/v1/a:b") {
        Resolution::Matched(m) => {
            assert_eq!(m.route.handler, 3, "expected the ** route");
            assert_eq!(m.captures().unwrap()[0].1, "a:b");
        }
        other => panic!("expected a match, got {other:?}"),
    }
}

#[test]
fn a_colon_a_verb_route_claims_is_peeled() {
    let t = RouteTable::new(TABLE);
    match t.resolve("POST", "/v1/op1:cancel") {
        Resolution::Matched(m) => {
            assert_eq!(m.route.handler, 2);
            assert_eq!(m.verb, "cancel");
            assert_eq!(m.captures().unwrap()[0].1, "op1");
        }
        other => panic!("expected a match, got {other:?}"),
    }
}

#[test]
fn malformed_capture_surfaces_the_field_name() {
    // The path matched and the value is what is wrong, so this is a 400 with a
    // FieldViolation, not a 404.
    let t = RouteTable::new(TABLE);
    match t.resolve("GET", "/v1/shelves/s1/books/b%2") {
        Resolution::Matched(m) => {
            let err = m.captures().unwrap_err();
            assert_eq!(err.field, "name");
            assert_eq!(err.kind, DecodeError::Truncated);
        }
        other => panic!("expected a match, got {other:?}"),
    }
}
