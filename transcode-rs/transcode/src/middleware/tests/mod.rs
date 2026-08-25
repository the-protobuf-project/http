//! Middleware tests.

// Tests assert by panicking, so `unwrap` is the correct idiom here and the
// workspace lints that forbid it in library code do not apply.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    unreachable_pub
)]

mod auth;
mod deadline;
mod fixtures;
mod headers;
mod metadata;
mod policies;
mod selector;
mod stack;
