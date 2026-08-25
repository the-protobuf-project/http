//! Route executor tests.
//!
//! The matching cases mirror `protokit/service/httprule/httprule_test.go` case
//! for case. The generator compiles a template once and every runtime executes
//! the result, so the only thing keeping the implementations from drifting is
//! that both are held to the same corpus.

// Tests assert by panicking, so `unwrap` is the correct idiom here and the
// workspace lints that forbid it in library code do not apply.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    unreachable_pub
)]

mod fixtures;
mod matching;
mod resolution;
