# transcode-build

Build-time integration for [`transcode`](../transcode).

A `build.rs` shim that invokes `protoc-gen-http` beside `tonic-build` and wires
the generated route tables into your crate.

The generator itself is Go, because that is where the protobuf ecosystem lives:
`protogen`, the `google.api.*` extension types, buf, and api-linter. This crate
locates the plugin binary and drives it; it does not reimplement it.

See the [README](../../README.md) for why templates are compiled
at build time rather than parsed at runtime.
