// Package music is the Go proof of concept: the same catalog the Rust example
// serves, over the route table protoc-gen-http emitted for this runtime.
//
// It exists to prove two things at once. That the Go runtime serves the
// protocol — routing, binding, negotiation, the AIP-193 envelope, and the
// no-false-2xx rule — and that it serves it identically to the Rust one, which
// the conformance tests assert against fixtures both runtimes are checked
// against.
//
// The service behind it is an in-memory catalog rather than a real gRPC backend.
// The point is the HTTP surface, and a real backend would only obscure whether
// that surface is correct.
package music
