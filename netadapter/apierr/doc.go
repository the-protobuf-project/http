// Package apierr is the AIP-193 error model.
//
// Every failure the adapter produces — routing, negotiation, binding,
// validation, or an RPC's own status — becomes an [Error] and is rendered
// through one place. That single funnel is the structural fix for the bug that
// motivated the project: grpc-gateway renders unary errors, stream errors and
// routing errors through three different paths, and they disagree about both
// the status and the body shape.
//
// The model is protocol-shaped rather than HTTP-shaped by accident: an [Error]
// carries a canonical google.rpc code, a set of google.rpc details, and the HTTP
// status that code projects to. An adapter over another protocol reuses the code
// and the details and projects them its own way, which is why this is not called
// "httperr".
//
// See README §5 for the normative shape.
package apierr
