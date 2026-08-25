// Package transcode fronts a gRPC service with an AIP-native HTTP/JSON surface.
//
// # Why "transcode"
//
// It is what the annotation calls itself. google.api.http describes HTTP/JSON
// transcoding, AIP-127 specifies it, and this package is an implementation of
// it: a request arrives over HTTP, is resolved against a table compiled from
// the service's own protos, and is handed to a dispatcher as a typed call.
//
// Naming the package after the job means an import path says what the code
// does. "Gateway" was the alternative and it is worse twice over — it names a
// topology rather than a behaviour, and it is the name of the project this one
// exists to disagree with.
//
// [Handler] is an [net/http.Handler] and nothing more. It owns no listener and
// no lifecycle, so it mounts into an existing mux and inherits whatever the
// deployment already does about TLS, timeouts and shutdown.
//
// # Two properties
//
//   - Nothing here parses protobuf. Path templates, field paths, validation
//     rules and response sets are compiled by protoc-gen-http at build time;
//     this package executes a table. Two runtimes generated from one IR
//     therefore cannot disagree about what a request means.
//   - A failed RPC is never reported as a success. Unary responses are fully
//     encoded before the status line is written, and a stream defers its header
//     until the first message or termination. See README §6.2.
//
// # Layout
//
// What is HTTP-specific lives in the packages that say so: route executes path
// templates, codec negotiates media types, stream frames a streaming response.
// The rest — the method classification, the error model, the phase pipeline —
// is protocol-neutral, which is what a second frontend over the same protos
// would reuse.
//
// # Relationship to grpc-gateway
//
// This is not a port of grpc-gateway. It follows the AIP corpus where the two
// disagree — most visibly in the error envelope (AIP-193), in payload
// validation, and in never reporting a failed RPC as a 200. The wire behaviour
// is specified in the repository README and is shared with the Rust runtime,
// which the conformance tests hold both to.
package transcode
