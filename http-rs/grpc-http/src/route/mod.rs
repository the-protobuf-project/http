//! The route executor.
//!
//! This module contains no parser. A `google.api.http` template is parsed and
//! compiled by `protoc-gen-http` at build time; what reaches the runtime is the
//! flattened result — a positional sequence of [`Match`] segments plus the
//! [`Capture`] spans that slice values out of a matched path. Matching is a
//! positional walk with no backtracking, because the compiler guarantees a
//! `**` can only appear last.
//!
//! The consequence worth stating: two runtimes generated from the same IR
//! cannot disagree about what a template means, because neither one interprets
//! templates. Both execute the same table, and the conformance suite holds them
//! to the Go reference implementation in
//! `protokit/service/httprule`.
//!
//! See README §1 for the normative rules this implements.

mod decode;
// Named for the type it defines; the parent module is the public facade.
#[allow(clippy::module_inception)]
mod route;
mod segment;
mod split;
mod table;

#[cfg(test)]
mod tests;

pub use decode::{CaptureError, DecodeError, decode_segment};
pub use route::Route;
pub use segment::{Capture, Match, TO_END};
pub use split::split_path;
pub use table::{Resolution, RouteMatch, RouteTable};
