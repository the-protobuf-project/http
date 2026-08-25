//! Compiled routes shared by the matching and resolution tests.
//!
//! These are written by hand in the form `protoc-gen-http` emits: `static`
//! arrays of `Match` and `Capture`, with no allocation and no parsing.

use crate::route::{Capture, Match, Route, TO_END};

/// `/v1/{name=shelves/*/books/*}` — a multi-segment capture.
pub static SHELVES_BOOKS: &[Match] = &[
    Match::Literal("v1"),
    Match::Literal("shelves"),
    Match::Single,
    Match::Literal("books"),
    Match::Single,
];

/// The `name` capture spanning all four segments of [`SHELVES_BOOKS`].
pub static CAP_NAME_1_5: &[Capture] = &[Capture {
    field: &["name"],
    json: "name",
    start: 1,
    end: 5,
}];

/// `/v1/{parent=shelves/*}/books` — a capture followed by a literal.
pub static PARENT_BOOKS: &[Match] = &[
    Match::Literal("v1"),
    Match::Literal("shelves"),
    Match::Single,
    Match::Literal("books"),
];

/// The `parent` capture of [`PARENT_BOOKS`].
pub static CAP_PARENT_1_3: &[Capture] = &[Capture {
    field: &["parent"],
    json: "parent",
    start: 1,
    end: 3,
}];

/// `/v1/{name=**}` — an unbounded capture.
pub static V1_MULTI: &[Match] = &[Match::Literal("v1"), Match::Multi];

/// The `name` capture of [`V1_MULTI`], running to the end of the path.
pub static CAP_NAME_TO_END: &[Capture] = &[Capture {
    field: &["name"],
    json: "name",
    start: 1,
    end: TO_END,
}];

/// `/v1/{name}` — also the base for `/v1/{name}:cancel`.
pub static V1_SINGLE: &[Match] = &[Match::Literal("v1"), Match::Single];

/// The `name` capture of [`V1_SINGLE`].
pub static CAP_NAME_1_2: &[Capture] = &[Capture {
    field: &["name"],
    json: "name",
    start: 1,
    end: 2,
}];

/// `/v1/{name=shelves/*}` — a two-segment capture.
pub static SHELVES_ONE: &[Match] = &[
    Match::Literal("v1"),
    Match::Literal("shelves"),
    Match::Single,
];

/// The `name` capture of [`SHELVES_ONE`].
pub static CAP_NAME_1_3: &[Capture] = &[Capture {
    field: &["name"],
    json: "name",
    start: 1,
    end: 3,
}];

/// Builds a route the way the generator emits one.
pub const fn route(
    method: &'static str,
    segments: &'static [Match],
    verb: &'static str,
    captures: &'static [Capture],
    template: &'static str,
    handler: usize,
) -> Route {
    Route {
        method,
        segments,
        verb,
        captures,
        template,
        handler,
    }
}

/// Splits a path the way README §1.2 step 2 requires: on the raw bytes,
/// dropping only the empty piece the leading slash produces.
pub fn split(path: &str) -> Vec<&str> {
    path.strip_prefix('/').unwrap_or(path).split('/').collect()
}

/// Matches and decodes in one step, for tests that assert on captured values.
pub fn matched(r: &Route, path: &str, verb: &str) -> Option<Vec<(&'static str, String)>> {
    let segs = split(path);
    if !r.matches(&segs, verb) {
        return None;
    }
    Some(
        r.captures
            .iter()
            .map(|c| (c.json, r.capture(c, &segs).unwrap().into_owned()))
            .collect(),
    )
}
