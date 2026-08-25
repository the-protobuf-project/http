// Package netadapter fronts a gRPC service with an AIP-native HTTP/JSON
// surface.
//
// # Why "adapter" and not "gateway"
//
// The name is the design. What this package does is adapt one network protocol
// onto a service defined in another: a request arrives over HTTP, is resolved
// against a table compiled from the service's own protos, and is handed to a
// dispatcher as a typed call. Nothing in that shape is specific to HTTP. The
// route table, the method classification, the error model and the phase
// pipeline are the same machinery an MCP adapter over the same protos needs, so
// naming this one "gateway" would have made the general thing sound like the
// HTTP special case.
//
// What is HTTP-specific lives in the packages that say so: [route] executes
// path templates, [codec] negotiates media types, [stream] frames a streaming
// response. The adapter itself is the pipeline that joins them.
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
// The adapter is an [net/http.Handler], so it serves over HTTP/1.1 and HTTP/2
// without knowing which it is on, and mounts inside an existing mux.
//
// # Relationship to grpc-gateway
//
// This is not a port of grpc-gateway. It follows the AIP corpus where the two
// disagree — most visibly in the error envelope (AIP-193), in payload
// validation, and in never reporting a failed RPC as a 200. The wire behaviour
// is specified in the repository README and is shared with the Rust runtime,
// which the conformance tests hold both to.
//
// [route]: https://pkg.go.dev/github.com/the-protobuf-project/grpc-gateway-rs/netadapter/route
// [codec]: https://pkg.go.dev/github.com/the-protobuf-project/grpc-gateway-rs/netadapter/codec
// [stream]: https://pkg.go.dev/github.com/the-protobuf-project/grpc-gateway-rs/netadapter/stream
package netadapter
