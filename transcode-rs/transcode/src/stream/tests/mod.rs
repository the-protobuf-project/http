//! Streaming tests.
//!
//! README §6.2 is the behaviour the whole project exists to get right,
//! and it is easy to implement almost-correctly, so it is tested from both
//! ends: that a pre-commit failure keeps its real status, and that a post-commit
//! failure is truncated rather than closed cleanly.

// Tests assert by panicking, so `unwrap` is the correct idiom here and the
// workspace lints that forbid it in library code do not apply.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    unreachable_pub
)]

mod framing;
mod no_false_2xx;
mod trailers;

use crate::error::{Code, Error};

/// The API domain used throughout.
pub const DOMAIN: &str = "music.example.com";

/// A `PERMISSION_DENIED`, standing in for the common pre-commit failure.
pub fn denied() -> Box<Error> {
    Box::new(
        Error::new(Code::PermissionDenied, "No access to that artist.").ensure_error_info(DOMAIN),
    )
}

/// An `INTERNAL`, standing in for a backend dying mid-stream.
pub fn exploded() -> Box<Error> {
    Box::new(Error::new(Code::Internal, "Backend went away.").ensure_error_info(DOMAIN))
}

/// Encodes an error the way the JSON codec would.
pub fn encode(err: &Error) -> Vec<u8> {
    serde_json::to_vec(&err.to_json()).unwrap()
}
