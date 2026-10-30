//! Gateway tests: the whole pipeline, driven through `Gateway::handle`.

// Tests assert by panicking, so `unwrap` is the correct idiom here and the
// workspace lints that forbid it in library code do not apply.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    unreachable_pub
)]

mod fixtures;
mod pipeline;
mod query;
mod streaming;
