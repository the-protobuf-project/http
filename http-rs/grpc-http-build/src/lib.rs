//! Build-time integration for [`grpc-http`].
//!
//! Invokes `protoc-gen-http` beside `tonic-build` from a `build.rs`, and wires
//! the route tables it emits into the crate being built.
//!
//! The generator is a Go binary. That is deliberate: `google.api.http` parsing,
//! AIP annotation reading, route-conflict detection, and `OpenAPI` emission all
//! happen once, in the language where the protobuf ecosystem actually lives,
//! and every runtime consumes the result. See the README
//!
//! [`grpc-http`]: https://docs.rs/grpc-http

// Implementation follows once the generator's output format is fixed; see
// the README for the intended layout.
