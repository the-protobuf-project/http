//! Error model tests.
//!
//! Several of these pin behaviour that grpc-gateway gets wrong; where that is
//! the point of the test, the comment says which of its files does what.

// Tests assert by panicking, so `unwrap` is the correct idiom here and the
// workspace lints that forbid it in library code do not apply.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    unreachable_pub
)]

mod details;
mod envelope;
mod headers;

/// The API domain used throughout, standing in for a real service's.
pub const DOMAIN: &str = "library.example.com";
