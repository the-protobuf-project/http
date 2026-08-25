# protoc-gen-http

The generator. Reads `google.api.http` and the AIP vocabulary once, in Go, and
emits a route table each runtime executes.

Nothing downstream parses a path template or reads a descriptor. That is what
keeps the Rust, Go, and Python gateways from disagreeing about what a request
means — none of them interprets a template, they all execute the same compiled
table.

## Use

```yaml
# buf.gen.yaml
version: v2
plugins:
  - local: protoc-gen-http
    out: src/generated
    opt:
      - domain=music.example.com   # required: the AIP-193 error domain
      - lang=rust
```

`domain` is required and has no default. It is stamped into every error response
and cannot be derived from the protos, which declare no such thing.

## What it guarantees

**An ambiguous route table fails the build.** Two bindings that overlap without
one dominating are a generation error naming both and an example path that
matches each. grpc-gateway resolves such a pair by registration order, at
request time, with no report either way.

**The emitted table is sorted most-specific-first**, so a runtime scans linearly
and takes the first match.

**Output is deterministic.** Generated code is committed, so a build that
reordered it would produce a diff on every run and make a real change impossible
to spot.

## Targets

| Language | Status |
| --- | --- |
| `rust` | emits the `transcode` route table |
| `go`, `python`, `openapi` | not yet built; the IR they consume is complete |

## Layout

| Path | What |
| --- | --- |
| `gateway/` | The factory model: the service IR plus build options |
| `target/rust/` | The Rust emitter — view construction, identifiers, templates |
| `cmd/protoc-gen-http/` | The binary |

The IR itself lives in
[`protokit/service`](https://github.com/the-protobuf-project/protokit), so a
second generator can consume it without depending on this one.
