# music-example

The proof of concept: a music catalog served over HTTP/1.1 and HTTP/3 by
[`transcode`](../../transcode-rs/transcode), from the AIP-annotated protos in
[`examples/protobuf`](../protobuf).

## Run it

```sh
cargo run -p music-example --features http3 --bin music-server
```

| Endpoint | Protocol | TLS |
| --- | --- | --- |
| `http://127.0.0.1:8080` | HTTP/1.1 | none |
| `https://127.0.0.1:8443` | HTTP/1.1 | TLS 1.3 |
| `https://127.0.0.1:8443` | HTTP/3 over QUIC (UDP) | TLS 1.3, always |

There is no plaintext HTTP/3 row and there cannot be: QUIC embeds TLS 1.3 in
its transport handshake, so an unencrypted HTTP/3 connection is not something
the protocol can express.

The TLS listeners use a self-signed certificate generated at startup, so no key
material is committed and every run gets a fresh one.

## What it demonstrates

**The template shapes a general-purpose router cannot express.**

```sh
curl http://127.0.0.1:8080/v1/artists/miles/tracks/so-what      # multi-segment capture
curl http://127.0.0.1:8080/v1/artists/miles/tracks              # capture then literal
curl -X POST -d '{}' -H 'Content-Type: application/json' \
     'http://127.0.0.1:8080/v1/artists/miles/tracks/so-what:withdraw'
```

`matchit` rejects the first two outright. It *accepts* the third and folds
`:withdraw` into the path variable, so `name` binds to
`artists/miles/tracks/so-what:withdraw` — a silent corruption worse than a
rejection. Here the verb is peeled and `name` is correct.

**protojson, exactly.** `monthlyListeners` is a JSON **string** because it is an
`int64`; `duration` is `"545s"`; `availability` is `"AVAILABILITY_STREAMING"`,
the enum's name rather than its number.

**AIP-193 errors, with the HTTP status in `code`.**

```sh
curl -i http://127.0.0.1:8080/v1/artists/nobody   # 404, {"error":{"code":404,…}}
curl -i -X PUT http://127.0.0.1:8080/v1/artists/miles  # 405 + Allow, not 501
```

grpc-gateway reports the *gRPC* code in that field, and turns the `405` into a
`501` by routing it through `UNIMPLEMENTED`.

**Streaming, and the no-false-2xx rule.**

```sh
curl -i 'http://127.0.0.1:8080/v1/artists/miles/tracks:watch'          # JSON array
curl -i 'http://127.0.0.1:8080/v1/artists/miles/tracks:watch?alt=sse'  # SSE
curl -i 'http://127.0.0.1:8080/v1/artists/nobody/tracks:watch'         # 404, not 200
```

The last one is the point. The stream fails before producing a message, so the
status line was never committed and the failure keeps its real `404` with an
ordinary error body. grpc-gateway writes `200` when the stream opens and appends
an error chunk, so a client reading only the status cannot tell the difference.

A failure *after* the first message cannot be given a real status — the header is
spent — so it emits an in-band error frame, sets `grpc-status` trailers, and
truncates the body. See README §6.2

**A typo is rejected, not ignored.**

```sh
curl -X PATCH -H 'Content-Type: application/json' \
     -d '{"biogrpahy":"typo"}' http://127.0.0.1:8080/v1/artists/miles
```

Returns `400` with a `BadRequest.FieldViolation` naming `biogrpahy`.

## Layout

| Path | What |
| --- | --- |
| `src/generated/` | Emitted by `protoc-gen-http` from the protos. Regenerate with `just gen-rust`; `just check-gen` fails if it is stale. Do not edit. |
| `src/handler/` | The request pipeline: route, bind, decode, call, encode. |
| `src/store/` | An in-memory catalog returning `tonic::Status`, standing in for the real service. |
| `src/serve/` | The listeners. Small, because the handler is transport-neutral. |

## Tests

```sh
cargo test -p music-example --features http3
```

`tests/http3.rs` drives a real `h3` client over QUIC — system `curl` on macOS
has no HTTP/3 support — and asserts it reaches the same handler with the same
bytes as HTTP/1.1.
