//! Codec and negotiation tests.

// Tests assert by panicking, so `unwrap` is the correct idiom here and the
// workspace lints that forbid it in library code do not apply.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    unreachable_pub
)]

mod accept;
mod fixtures;
mod negotiate;

#[cfg(feature = "json")]
mod json;
