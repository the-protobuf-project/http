//! A music catalog served over HTTP by `transcode`.
//!
//! This is the project's proof of concept. It proves three things the design
//! claims but had not yet demonstrated:
//!
//! 1. **The four template shapes route correctly** — including the custom verb
//!    a general-purpose router silently mis-binds.
//! 2. **Failures produce AIP-193 envelopes** with the HTTP status in `code`,
//!    through one funnel regardless of where they originated.
//! 3. **One handler serves HTTP/1.1 and HTTP/3 alike**, with and without TLS,
//!    because it is a plain function over a request rather than anything tied
//!    to a transport.
//!
//! Everything under [`generated`] is what `protoc-gen-http` will emit; writing
//! it by hand first fixes the output format the generator has to produce.

pub mod generated;
pub mod handler;
pub mod model;
pub mod requests;
pub mod serve;
pub mod store;
