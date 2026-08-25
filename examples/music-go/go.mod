module github.com/the-protobuf-project/http/examples/music-go

go 1.26.4

require (
	github.com/the-protobuf-project/http/netadapter v0.0.0
	google.golang.org/genproto/googleapis/api v0.0.0-20260819154853-08b0e4226688
	google.golang.org/protobuf v1.36.12
)

// The runtime is in this repository and unpublished, so the example reaches it
// by path. A replace directive as well as the workspace, so the example still
// builds when someone takes this directory on its own — it is what proves the
// generated table compiles against the runtime it was emitted for.
replace github.com/the-protobuf-project/http/netadapter => ../../netadapter
