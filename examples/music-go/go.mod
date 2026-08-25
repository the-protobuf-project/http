module github.com/the-protobuf-project/grpc-gateway-rs/examples/music-go

go 1.26.4

require (
	github.com/the-protobuf-project/grpc-gateway-rs/netadapter v0.0.0
	google.golang.org/genproto/googleapis/api v0.0.0-20260819154853-08b0e4226688
	google.golang.org/protobuf v1.36.12
)

// The runtime is in this repository and unpublished, so the example reaches it
// by path. A replace directive rather than the workspace, because the workspace
// is gitignored and this example must build from a clean checkout — it is what
// proves the generated Go table compiles against the runtime it was emitted for.
replace github.com/the-protobuf-project/grpc-gateway-rs/netadapter => ../../netadapter
