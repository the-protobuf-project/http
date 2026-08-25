# music-go

The Go proof of concept: the same catalog `music-rs` serves, over the route
table `protoc-gen-http` emitted for the [`transcode`](../../transcode-go) runtime
from the same protos.

```sh
just run-go
curl http://127.0.0.1:8080/v1/artists/miles/tracks/so-what
```

Nothing here is hand-written that a generator can produce. `protoc-gen-go` emits
the messages into `gen/` and `protojson` marshals them, so the JSON mapping is
the protobuf runtime's rather than a set of struct tags that resemble it;
`protoc-gen-http` emits the route table into `routes/`. What is left is the
service — an in-memory catalog — and the handlers that bind a call to it, which
is the shape the Go target will emit once it generates handlers too.

## What the tests are for

Each file asserts a rule from the [protocol](../../README.md#the-protocol) that
the Rust runtime is held to as well, so a divergence between the two shows up as
a failing assertion rather than as drift nobody looks for.

| File | Rule |
| --- | --- |
| `conformance_test.go` | the AIP-193 envelope: `code` is the HTTP status, exactly one `ErrorInfo`, `405` keeps its status and carries `Allow` |
| `routing_test.go` | multi-segment captures, custom verbs, the `%2F` exception, unknown query parameters |
| `streaming_test.go` | both halves of the no-false-2xx rule |
| `middleware_test.go` | selector dispatch on the AIP pattern, and the builtins' externally visible behaviour |

## Seeing the no-false-2xx rule for real

The tests assert it against a recorder. Over a real socket it looks like this:

```sh
just run-go

# Fails before the first message: the status line was never committed, so the
# failure keeps its real status and an ordinary error body.
curl -s -o /dev/null -w '%{http_code}\n' \
  'http://127.0.0.1:8080/v1/artists/miles/tracks:watch?failAfter=0'
# 503

# Fails after: the status is spent, so the body is truncated instead. curl
# exits 18 (partial file) even though the status line said 200 — which is the
# point. grpc-gateway closes cleanly here and the client sees success.
curl -s 'http://127.0.0.1:8080/v1/artists/miles/tracks:watch?failAfter=1'; echo "exit: $?"
# exit: 18
```

`failAfter` exists for exactly this. A real service has no such parameter — and
a real service also cannot be asked to fail on cue, so a rule tested only by
unit tests is a rule no transport is ever held to.
