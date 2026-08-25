# transcode

An AIP-native HTTP/JSON surface for a gRPC service, driven by `google.api.http`
annotations.

The crate executes a route table compiled by `protoc-gen-http`. It parses no
protobuf, reads no descriptors, and interprets no path templates — all of that
happens at build time, which is what keeps this and
[`transcode-go`](../../transcode-go) from disagreeing about what a request
means.

The name is the job: `google.api.http` and AIP-127 call this HTTP/JSON
transcoding.

See the [README](../../README.md) for the wire contract and how the
pieces fit together.

## Features

| Feature | Default | What it adds |
| --- | --- | --- |
| `json` | yes | The protojson codec |
| `proto` | no | The binary protobuf codec |
| `tls` | no | In-process TLS termination via rustls |
| `http3` | no | HTTP/3 over QUIC via h3 and quinn |
| `cel` | no | Runtime protovalidate CEL evaluation |
| `full` | no | All of the above |

## Not grpc-gateway

This is not a port. Where [grpc-gateway] and the [AIP] corpus disagree, this
follows AIP: the AIP-193 error envelope, gateway-side validation, and a mid-
stream failure that a client can actually detect. the protocol, Divergences lists every
divergence.

[grpc-gateway]: https://github.com/grpc-ecosystem/grpc-gateway
[AIP]: https://google.aip.dev/
