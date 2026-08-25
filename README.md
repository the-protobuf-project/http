# http

An AIP-native HTTP/JSON surface for gRPC services, generated from
`google.api.http` annotations. One generator, several runtimes, and an OpenAPI
document that cannot drift from the routes it describes.

It is not a port of [grpc-gateway](https://github.com/grpc-ecosystem/grpc-gateway).
Where that project and the [AIP](https://google.aip.dev/) corpus disagree, this
follows AIP — most visibly in the error envelope, in payload validation, and in
never reporting a failed RPC as a `200`.

**Status: in development.** The Rust and Go runtimes and both codegen targets
work end to end, against the same route table generated from the same protos.
The OpenAPI target is not built yet, and the Python runtime is deferred.

## Contents

- [Why not grpc-gateway](#why-not-grpc-gateway)
- [Quick start](#quick-start)
- [Repository layout](#repository-layout)
- [Building from a clean checkout](#building-from-a-clean-checkout)
- [Architecture](#architecture)
- [The protocol](#the-protocol)
- [Divergences from grpc-gateway](#divergences-from-grpc-gateway)
- [Testing](#testing)
- [CI and releases](#ci-and-releases)
- [Non-goals](#non-goals)

## Why not grpc-gateway

grpc-gateway is the reference implementation of transcoding, and this is not a
port of it. Three defects motivated starting over, all verifiable in its own
source:

**Failed RPCs are reported as successes.** `runtime/handler.go:229`,
`handleForwardResponseStreamError`, sets a status only `if !wroteHeader`. Once
one message has been written the header is committed, so a stream that fails
midway emits `{"error":{…}}` as another chunk of a `200` and closes the body
cleanly. A client watching status codes cannot distinguish it from success.
`runtime/handler.go:86` wraps every streamed message in `{"result": …}`, so the
body shape does not change either.

**The error body is not AIP-193.** `runtime/errors.go:105`,
`DefaultHTTPErrorHandler`, marshals `s.Proto()` directly, producing
`{"code": 3, "message": "…"}`. That `code` is the gRPC code, not the HTTP
status. AIP-193 specifies `{"error": {"code": 400, "status": "INVALID_ARGUMENT",
…}}`. Even when the HTTP status is right, the body reports a number that is not
an HTTP status.

**Routing errors do not survive.** `runtime/errors.go:196` maps `405 Method Not
Allowed` through `codes.Unimplemented` and back out as `501`.

Add to that: no payload validation of any kind, unknown query parameters
silently discarded, and an OpenAPI generator that documents only `200` and a
`default`. Each is individually fixable with a hook; together they describe a
gateway built for gRPC compatibility rather than for API correctness.

What this project takes from it is real and substantial: the path-template
opcode design, the metadata-matcher model, the field-path query binding idea,
and a decade of edge cases found the hard way.

## Quick start

```sh
just gen              # build the plugin, generate both examples' tables
just ci               # protos, Rust, Go, the generator, and a staleness check

just run-go           # the Go example on http://127.0.0.1:8080
cargo run -p music-example --features http3 --bin music-server   # the Rust one
```

Both serve the same catalog from [`examples/`](examples), over the same route
table. The Rust example adds two more transports:

| Endpoint | Protocol | TLS |
| --- | --- | --- |
| `http://127.0.0.1:8080` | HTTP/1.1 | none |
| `https://127.0.0.1:8443` | HTTP/1.1 | TLS 1.3 |
| `https://127.0.0.1:8443` | HTTP/3 over QUIC (UDP) | TLS 1.3, always |

There is no plaintext HTTP/3 row and there cannot be: QUIC embeds TLS 1.3 in its
transport handshake.

```sh
curl http://127.0.0.1:8080/v1/artists/miles/tracks/so-what      # multi-segment capture
curl -X POST -d '{}' -H 'Content-Type: application/json' \
     'http://127.0.0.1:8080/v1/artists/miles/tracks/so-what:withdraw'
curl -i  http://127.0.0.1:8080/v1/artists/nobody                # 404 with an AIP-193 body
curl -i 'http://127.0.0.1:8080/v1/artists/nobody/tracks:watch'  # 404, not a 200 with an error chunk
```

## Repository layout

```
http/
  Cargo.toml            Rust workspace root
  Justfile              every task; `just` lists them
  go.work               the four Go modules, so root-level commands resolve

  .github/workflows/    CI, release, and Dependabot auto-merge
  scripts/
    conformance.sh      asks both runtimes the same questions
    lib/compare.sh      how two answers are compared

  plugin/               protoc-gen-http, the Go generator (its own module)
    ir/                 the service IR as a protokit factory model
    target/table/       the language-neutral route-table view, shared by targets
    target/rust/        the Rust emitter
    target/golang/      the Go emitter
    cmd/protoc-gen-http/

  transcode-rs/
    transcode/          the Rust runtime, a tower::Service
    transcode-build/    build.rs integration

  transcode-go/         the Go runtime, an http.Handler (its own module)
  http-py/              deferred

  examples/
    protobuf/           the AIP-annotated music protos, and buf config
    music-rs/           the Rust proof of concept
      src/generated/    emitted by protoc-gen-http; do not edit
    music-go/           the Go proof of concept (its own module)
      gen/              messages, emitted by protoc-gen-go; do not edit
      routes/           the route table, emitted by protoc-gen-http; do not edit
```

Neither example hand-writes its message types. `protoc-gen-go` emits them and
`protojson` marshals them, so the JSON mapping in [§4.1](#41-json-mapping) comes
from the protobuf runtime rather than from struct tags that only resemble it.

The IR the generator builds lives in
[protokit](https://github.com/the-protobuf-project/protokit), not here, so a
second generator can consume it without depending on this repository.

## Building from a clean checkout

Everything builds from a clean checkout. The generator depends on
[protokit](https://github.com/the-protobuf-project/protokit) `v1.3.0`, which is
the release that carries the `service` package this project contributed.

```sh
cargo build --workspace --all-features   # the Rust runtime and example
go build ./...                           # the generator, the Go runtime, the Go example
```

`go.work` is committed. There are four Go modules here — the generator, the Go
runtime, the Go example, and the plugin — and the workspace is what lets a
command from the repository root resolve all of them, so `just run-go` and
`go test ./...` work without a per-module `cd`. Every member is inside this
repository: a workspace applies the union of its members' `replace` directives
to every module in it, and one pointing at a gitignored sibling is what used to
make a clean checkout fail.

---

# Architecture

## Shape

```
.proto  ─┐                                   ┌─► rust target    ──► transcode-rs
         ├─► protoc-gen-http ──► Service IR ──┼─► go target      ──► transcode-go
buf.yaml ┘      (Go, on protokit)             └─► openapi target ──► openapi.yaml
```

One frontend, several targets, one IR. The frontend is Go because that is where the
protobuf ecosystem lives — `protogen`, the `google.api.*` extension types, buf,
and api-linter. The runtimes serve requests and do no schema work at all.

The dividing line: **everything that requires understanding protobuf happens at
build time; everything at runtime is table-driven.** A runtime never parses a
path template, never reads a descriptor, and never resolves a field path. It
executes a route table and calls typed setters the generator emitted. That is
what keeps several runtimes in different languages from disagreeing about what a
request means — none of them decides.

## Why protokit, and which parts

protokit is the org's proto-frontend library. Worth being precise: **its
original IR is a database IR** — `Database`, `Table`, `Column`, foreign keys,
indexes. There was no service IR to inherit, so this project contributed one,
and it shipped in protokit `v1.3.0` as `protokit/service`. What this repository
reuses beyond it is the generic half: `factory` (`Source`/`Target`/`Registry`),
`header`, `naming`, `templates`, `golden`, `manifest`, and
`types.ClassifyField`.

The IR lives there rather than here so a second generator — an MCP adapter over
the same protos, say — can consume it without depending on this repository.

## The IR

Codec-neutral, transport-neutral, language-neutral. Two properties are
load-bearing:

**`QueryParams` is computed, not discovered.** grpc-gateway walks fields
reflectively at request time with a filter marking what the path and body
already bound. `prost` has no reflection at all, and reaching for Go's would
mean the runtime carrying a descriptor set it otherwise never needs, so the
subtraction happens once, at build time, and every runtime receives an explicit
list.

**`Route` is compiled, not a string.** See below.

The IR also carries more than routing needs — validation rules, resource
patterns, singular and plural names, per-binding response sets — because the
OpenAPI target needs them and it reads the same IR.

## Routing

`matchit`, axum's router, cannot express `google.api.http`:

```
/v1/{name=shelves/*/books/*}   → Err(InvalidParam)
/v1/{parent=shelves/*}/books   → Err(InvalidParam)
/v1/{name=**}                  → Err(InvalidParam)
/v1/{name}:cancel              → Ok   // accepted, but ":cancel" becomes part of `name`
```

Three are rejected. The fourth is worse than rejected: accepted as an ordinary
route, silently binding `name` to the wrong value.

So the gateway carries its own matcher — and the decision that follows is where
this design departs from the obvious one: **the template is parsed and compiled
in Go, at build time, and the runtimes ship an executor, not a parser.**

The IR carries a flattened match sequence plus capture spans. Each target emits
that as a static table; matching is a positional walk. Roughly two hundred lines
per runtime, no grammar in sight. Three things follow:

**One grammar.** `google.api.http` is parsed once, by one implementation, tested
once.

**Route conflicts become compile errors.** grpc-gateway resolves overlapping
patterns by registration order, at runtime, silently. With the whole route set
in hand at build time, the generator fails on any pair that overlaps without one
dominating, naming both and an example path that matches each.

**AIP-aware path expansion.** Because the compiler also has the
`google.api.resource` patterns, it can expand `{name=shelves/*/books/*}` into
`shelves/{shelf}/books/{book}` for OpenAPI, turning an opaque single `{name}`
parameter into named ones.

The cost, stated plainly: `transcode-build` is not a pure-cargo `build.rs`; it
wraps the plugin binary. A future `prost-reflect` dynamic proxy would need a
runtime parser, and that is a separate mode.

## Codecs

Path captures and query parameters arrive as strings and are parsed by generated
typed setters, so the codec boundary is narrower than it looks: the request
body, the response body, and stream framing. Nothing else.

```rust
pub trait Codec: Send + Sync + 'static {
    fn name(&self) -> &'static str;              // the ?alt= selector
    fn media_types(&self) -> &'static [&'static str];
    fn framing(&self) -> Framing;
}
pub trait Encode<M>: Codec { fn encode(&self, m: &M, out: &mut BytesMut) -> Result<(), CodecError>; }
pub trait Decode<M>: Codec { fn decode(&self, b: &[u8]) -> Result<M, CodecError>; }
```

`Encode<M>` and `Decode<M>` are deliberately **not** object-safe. `Codec` carries
only metadata, so the registry can negotiate without knowing any message type;
the generated handler knows its concrete types and monomorphises the call, so
there is no dynamic dispatch per request. A new codec costs two generated impls
per message type and nothing at runtime.

protojson is a *mapping*, not a serializer — camelCase, enums as strings,
64-bit integers as strings — and those belong to the JSON codec, not to the
trait. A FlatBuffers codec would inherit none of them, and would be wrong if it
did.

## Middleware

Two planes, kept distinct, and the split is the same in both runtimes.

**The transport plane** is whatever the host ecosystem already has — `tower::Layer`
in Rust, a wrapped `http.Handler` in Go. Compression, TLS identity, body limits.
Nothing here duplicates it.

**The message plane** is everything needing the resolved method, the bound
message, or the typed response. A transport-plane layer cannot see any of it,
because routing has not happened when that layer runs:

```rust
pub trait Interceptor: Send + Sync + 'static {
    fn on_route(&self, cx: &mut RouteCx<'_>) -> Result<()> { Ok(()) }
    fn on_request(&self, cx: &mut CallCx<'_>) -> Result<()> { Ok(()) }
    fn on_response(&self, cx: &mut CallCx<'_>, parts: &mut ResponseParts) -> Result<()> { Ok(()) }
    fn on_complete(&self, cx: &CallCx<'_>, outcome: &Outcome<'_>) {}
}
pub trait InspectRequest<M>  { /* typed, opt-in, monomorphised by codegen */ }
pub trait InspectResponse<M> { /* … */ }
```

`Interceptor` is object-safe and covers the majority of policies, because authn,
authz, quota, audit, and tracing all key on *which* method was called rather
than on what it was sent. Payload access is the specialisation.

The Go runtime splits the same four phases into four optional interfaces —
`RouteHook`, `RequestHook`, `ResponseHook`, `CompleteHook` — because Go has no
defaulted methods, and an interceptor implementing none of them is rejected at
registration rather than silently never running.

Every failure exits through one `ErrorRenderer`. grpc-gateway has three separate
hooks — `WithErrorHandler`, `WithStreamErrorHandler`, `WithRoutingErrorHandler` —
which is *why* its unary, stream, and routing errors disagree about status and
body shape. One funnel makes that class of divergence unrepresentable.

### Selectors

```rust
gateway
    .layer(Recovery::default())
    .layer(Deadline::new(Duration::from_secs(30)))
    .layer_on(Auth::bearer(verifier, DOMAIN), Selector::Mutating)
    .layer_on(Quota::new(limits), Selector::Pattern(MethodPattern::List));
```

```go
transcode.New(routes.NewTable(), routes.NewRegistry(), service, routes.Domain,
    transcode.Use(builtin.NewRecovery(logger)),
    transcode.Use(builtin.NewDeadline(30*time.Second, domain)),
    transcode.UseFor(builtin.Bearer(verifier, domain), middleware.Mutating()),
    transcode.UseFor(builtin.NewRateLimit(limits, domain), middleware.Pattern(route.PatternList)),
)
```

`Mutating` resolves against the AIP pattern the generator emitted, so adding a
Create later is covered automatically — a policy written against a name prefix
would silently miss it. In Go the selection is resolved once per method when the
handler is built, since a selector is a predicate over a method table that is
fixed at generation time.

### What ships in the box

Mirroring [`go-grpc-middleware`](https://github.com/grpc-ecosystem/go-grpc-middleware),
plus the two grpc-gateway offers as mux options. Both runtimes ship all of them:

| Interceptor | Behaviour |
| --- | --- |
| `Recovery` | catches unwinds, emits `500` / `GATEWAY_PANIC`, never drops the connection |
| `Deadline` | `Grpc-Timeout` → RPC deadline, capped, mandatory default, `504` on expiry |
| `Auth` | pluggable verifier, `401` with a well-formed `WWW-Authenticate` |
| `RateLimit` | `429` with `QuotaFailure` + `RetryInfo` + `Retry-After` |
| `RealIp` | resolves the client behind N trusted proxies |
| `Validate` | typed, per-message, from the four sources below |
| `Idempotency` | AIP-155 `request_id` deduplication |
| `Logging` | one structured line per call, labelled by template |
| `Metrics` | bounded-cardinality metrics through a sink interface |
| `Health` | `WithHealthzEndpoint` / `WithHealthEndpointAt`; answers before routing, so it works when nothing else does |
| `Cors` | preflight and headers, `Allow-Methods` exact from the route table |

`retry` has no counterpart, deliberately. In go-grpc-middleware it is a *client*
interceptor, and retrying at the gateway would be wrong: the gateway cannot know
whether a method is idempotent, and replaying a non-idempotent one turns a
timeout into a duplicate write.

`RateLimit` and `Idempotency` take interfaces rather than implementations,
because a per-process token bucket silently permits N times the configured rate
across replicas, and a per-process request-id set lets a retry landing on
another replica execute twice.

### Covering grpc-gateway's option surface

All seventeen `ServeMuxOption` constructors have a counterpart, enumerated in the
`middleware` module docs. Two are deliberately not reproduced as-is:
`WithUnescapingMode`'s default decodes the whole path before routing, which lets
a `%2F` invent a segment boundary; and `WithDisablePathLengthFallback` guards a
retry that makes which route a request reached unpredictable, so the fallback is
off by default rather than on.

## Validation

Four sources — AIP-203 field behaviour, AIP-122/123 resource-name patterns,
`google.api.field_info` formats, and protovalidate CEL. Three compile to direct
code: a `REQUIRED` check is an `is_none()`, a resource pattern is a generated
segment matcher, a `UUID4` format is a generated parser. They cost nothing at
runtime and appear in OpenAPI as `required`, `pattern`, and `format`.

Only CEL needs an evaluator, and the generator lowers its constant subset
(`min_len`, `max_len`, `gt`, `lt`, `pattern`, `in`, `not_in`, `required`) too. A
build using expressions that need the evaluator without the feature enabled
**fails generation** rather than silently skipping the constraint — a validation
rule that quietly does nothing is worse than no validation.

Gateway-side validation is defence in depth, not a substitute for the service's
own: a service must still assume unvalidated input, because the gateway is not
the only way in. What it buys is a good error at the edge and a truthful OpenAPI
document.

## OpenAPI

A third target off the same IR, which is the only way the document and the
routes cannot drift. Its three requirements all need IR the runtimes never
touch, which is why the IR carries more than routing needs — see
[§9 OpenAPI v3](#9-openapi-v3).

## api-linter

api-linter runs in CI over the fixture protos, and the generator treats its
findings as a build input. The dependency runs the other way too: middleware
selectors dispatch on AIP method patterns, OpenAPI path expansion needs
`google.api.resource` patterns to be correct, validation trusts
`field_behavior`. All of that is only safe because the protos are linted. An
unlinted proto set degrades gracefully — `Custom` patterns, unexpanded paths, no
validation — but the good behaviour is earned, not assumed.

Note that buf's `STANDARD` lint category contradicts AIP on response naming:
AIP-131 says `GetArtist` returns `Artist`, buf wants `GetArtistResponse`.
`examples/protobuf/buf.yaml` excepts those rules and documents why.

## Decisions

| Decision | Choice | Why |
| --- | --- | --- |
| Codegen home | Go plugin on protokit | one frontend for four targets; the protobuf ecosystem is Go |
| Template parsing | build time only; runtimes execute a compiled table | one grammar, and conflicts become compile errors |
| IR home | `protokit/service` (v1.3.0) | a second generator — an MCP adapter, say — consumes it without depending on this repo |
| Go runtime name | `transcode`, not `gateway` | it names the job `google.api.http` and AIP-127 name, so the import path says what the code does; "gateway" names a topology, and is the project this one disagrees with |
| JSON semantics | protojson, no deviations | a generated client and the gateway must agree exactly |
| Error envelope | AIP-193 always | the convention the rest of the ecosystem reads |
| Streaming default | JSON array, SSE on `?alt=sse` / `Accept` | matches Google's own REST streaming |
| Stream header commit | deferred to the first message | a pre-output failure keeps its real status |
| Stream failure | error frame + trailers + abnormal termination | the only way a status-only client observes failure |
| Unknown query params | rejected | a typo in an update call should not be a silent no-op |
| Validation | on by default, four sources, gateway-side | no reason to forward a request known to be invalid |
| Client/bidi streaming | build error, not a broken handler | HTTP transcoding has no honest mapping for it |

---

# The protocol

Normative. This is what every runtime implements and what the conformance corpus
tests. The key words MUST, MUST NOT, SHOULD, SHOULD NOT and MAY are as in
RFC 2119 and RFC 8174.

Version `1`, advertised as `X-Gateway-Protocol: 1`. Additive changes — a new
codec, a new detail projection — do not bump it; any observable change to
routing, binding, errors, or streaming does.

## 1. Routing

### 1.1 Template grammar

```ebnf
Template  = "/" Segments [ Verb ] ;
Segments  = Segment { "/" Segment } ;
Segment   = "*" | "**" | Literal | Variable ;
Variable  = "{" FieldPath [ "=" Segments ] "}" ;
FieldPath = Ident { "." Ident } ;
Verb      = ":" Literal ;
```

A bare `{var}` is exactly `{var=*}`. Enforced at build time: `**` MUST be final;
no two captures may share a field path; a capture's leaf MUST be a scalar, enum,
or string-typed well-known type; nested captures are an error.

### 1.2 Segmentation and percent-decoding

Everything downstream depends on this, and it is where implementations most
often diverge.

1. Take the path **before any decoding**.
2. Split on `/`. A trailing `/` produces a final empty segment, which MUST NOT
   match `*`.
3. Match using the **raw** bytes for literal comparison.
4. **After** a binding is selected, decode each captured segment, then join a
   multi-segment capture with `/`.

Step 4 decodes every escape **except `%2F` and `%2f`, which are left as
written.** That exception is the whole rule: `/` separates the segments of an
AIP-122 resource name, so decoding it would make `/v1/shelves/a%2Fb` and
`/v1/shelves/a/b` both yield `name = "shelves/a/b"`, and nothing downstream
could tell a two-segment name holding a slash from a three-segment name.

A segment whose encoding is truncated (`%2`), non-hex (`%zz`), or decodes to
invalid UTF-8 is `400` with reason `MALFORMED_PATH`.

- `*` matches exactly one non-empty segment.
- `**` matches zero or more; its value is the decoded segments rejoined with `/`.
- Literals compare byte-exact and case-sensitively, on raw bytes.

### 1.3 Custom verbs (AIP-136)

A verb is a suffix on the final segment, split at that segment's last `:`.
Because `:` is legal in a resource id, the verb is only peeled when a registered
verb-bearing route claims it; otherwise the segment is retried whole. A gateway
MUST NOT strip a `:` suffix no route asked for.

### 1.4 Precedence and conflicts

Most specific first: a verb-bearing route outranks its verbless twin; then
segment by segment, `Literal` > `*` > `**`; then a longer literal prefix. If two
bindings remain indistinguishable **and** can match a common request, the
generator MUST fail the build, naming both and an example path.

### 1.5 Routing failures

| Condition | Status | Code |
| --- | --- | --- |
| No template matches | `404` | `NOT_FOUND` |
| Path matches, method does not | `405` | `UNIMPLEMENTED` |
| Path matches, verb unknown | `404` | `NOT_FOUND` |
| Body on a binding declaring none | `400` | `INVALID_ARGUMENT` |

A `405` MUST carry `Allow`. The status line MUST be `405`; it MUST NOT be
rewritten to `501` by a code round trip.

## 2. Request binding

In order; each stage MUST fail rather than overwrite an earlier one:

```
1. path captures  →  2. body  →  3. query params  →  4. validation
```

**Body.** Absent means no body is permitted. `"*"` means the whole message, and
a field already bound by the path MUST NOT appear in it. A field path targets
that field, which MUST be message-typed and non-repeated. `google.api.HttpBody`
is passed through verbatim, no codec involved.

**Query parameters.** Named by protojson path — `?book.displayName=Dune`.
Repeated fields take one parameter per element; `FieldMask` takes a comma-joined
list; enums accept the name or the number; `Timestamp` takes RFC 3339 and
`Duration` the `"1.5s"` form. Map fields are not bindable. An `OUTPUT_ONLY`
field is not bindable and supplying it is `400`.

**An unrecognised query parameter is `400`**, with a `BadRequest` detail naming
it. This is the opposite of grpc-gateway, which discards them — turning a typo
into a silent no-op on an update call.

**System parameters** are stripped before binding and never bound to fields:
`alt` (response codec), `fields` (AIP-157 partial response), `prettyPrint`, and
their `$`-prefixed aliases. Any other `$`-prefixed parameter is reserved and
MUST be rejected.

### 2.1 Validation

Four sources, all violations collected into one response:

| Source | Rejects |
| --- | --- |
| AIP-203 `field_behavior` | `REQUIRED` absent; `OUTPUT_ONLY` present; `IMMUTABLE` outside the update mask; `IDENTIFIER` in a Create body |
| AIP-122/123 resource names | a name not matching a declared `pattern` |
| `google.api.field_info` | `UUID4`, `IPV4`, `IPV6`, `IPV4_OR_IPV6` |
| protovalidate | `buf.validate` constraints |

```json
{
  "error": {
    "code": 400,
    "message": "Request contains 2 invalid fields.",
    "status": "INVALID_ARGUMENT",
    "details": [
      { "@type": "type.googleapis.com/google.rpc.BadRequest",
        "fieldViolations": [
          { "field": "book.displayName", "description": "must be between 1 and 63 characters",
            "reason": "VALUE_LENGTH" },
          { "field": "parent", "description": "must match pattern \"shelves/{shelf}\"",
            "reason": "RESOURCE_NAME_MALFORMED" } ] },
      { "@type": "type.googleapis.com/google.rpc.ErrorInfo",
        "reason": "INVALID_ARGUMENT", "domain": "library.example.com" }
    ]
  }
}
```

`field` is the protojson path, so it names what the client sent and what OpenAPI
documents.

## 3. Codec negotiation

**Request** — from `Content-Type`, parameters ignored. Unregistered is `415`. A
body-less request needs no codec.

**Response** — first match wins: `?alt=<name>`; then `Accept`, honouring quality
values, wildcards and ordering; then the request codec; then the registry
default (`json`). If `Accept` is present and nothing in it is registered, the
response is `406`. A gateway MUST NOT fall back to a codec the client excluded.

| `alt` | Media type | Framing |
| --- | --- | --- |
| `json` | `application/json` | JSON array |
| `sse` | `text/event-stream` | SSE — streaming only; unary rejects with `400` |
| `ndjson` | `application/x-ndjson` | line-delimited |
| `proto` | `application/x-protobuf` | length-prefixed |

## 4. Responses

The body is the response message, **not** wrapped in an envelope. A
`response_body:` field is emitted alone, as a standalone JSON value. `?fields=`
applies a field mask before encoding.

`200`, except: `201` for an AIP-133 Create on `POST`, with `Location` naming the
created resource; `202` for a pending `google.longrunning.Operation`; `204` when
the response is `google.protobuf.Empty` with no `response_body`.

### 4.1 JSON mapping

protojson, no deviations:

| Proto | JSON |
| --- | --- |
| `int64`, `uint64`, `fixed64`, `sfixed64`, `sint64` | **string** |
| `int32` and narrower | number |
| `float`, `double` | number; `"NaN"`, `"Infinity"` as strings |
| `bytes` | base64, standard alphabet, padded |
| `enum` | the value name; input also accepts the number |
| `Timestamp` | RFC 3339, UTC, `Z`, 0/3/6/9 fractional digits |
| `Duration` | decimal seconds with `s`, e.g. `"1.000340012s"` |
| `FieldMask` | comma-joined lowerCamelCase paths |
| `Any` | the message's fields plus `"@type"` |
| wrappers | the wrapped scalar, or `null` |

Field names are lowerCamelCase on output; input accepts both spellings. Defaults
are omitted unless `emit_defaults=true`. Unknown fields are rejected unless
`ignore_unknown_fields=true`.

The 64-bit-as-string rule is not optional. It is the most common source of
silent precision loss in JSON gateways, and the OpenAPI output declares it so
generated clients agree.

## 5. Errors

### 5.1 Envelope

Every non-2xx body is exactly this (AIP-193):

```json
{
  "error": {
    "code": 404,
    "message": "Book \"shelves/s1/books/b9\" not found.",
    "status": "NOT_FOUND",
    "details": [
      { "@type": "type.googleapis.com/google.rpc.ErrorInfo",
        "reason": "RESOURCE_MISSING", "domain": "library.example.com",
        "metadata": { "resource": "shelves/s1/books/b9" } }
    ]
  }
}
```

`code` is the **HTTP status**, not the gRPC code. `status` is the canonical
`google.rpc.Code` name. `details` MUST contain exactly one `ErrorInfo`; a
service returning none gets one synthesised.

### 5.2 Status mapping

| Code | HTTP | | Code | HTTP |
| --- | --- | --- | --- | --- |
| `OK` | 200 | | `ABORTED` | 409 |
| `CANCELLED` | 499 | | `OUT_OF_RANGE` | 400 |
| `UNKNOWN` | 500 | | `UNIMPLEMENTED` | 501 |
| `INVALID_ARGUMENT` | 400 | | `INTERNAL` | 500 |
| `DEADLINE_EXCEEDED` | 504 | | `UNAVAILABLE` | 503 |
| `NOT_FOUND` | 404 | | `DATA_LOSS` | 500 |
| `ALREADY_EXISTS` | 409 | | `PERMISSION_DENIED` | 403 |
| `FAILED_PRECONDITION` | 400 | | `UNAUTHENTICATED` | 401 |
| `RESOURCE_EXHAUSTED` | 429 | | | |

`FAILED_PRECONDITION` maps to `400`, not `412`, per AIP-193. A gateway MAY
promote it to `412` for an `If-Match` mismatch on an AIP-154 etag, the one case
where the HTTP semantics genuinely coincide.

### 5.3 Header projection

`RetryInfo.retry_delay` → `Retry-After` (rounded up). `Help.links` →
`Link: <url>; rel="help"`. On `401`, a well-formed challenge:

```
WWW-Authenticate: Bearer realm="library.example.com", error="invalid_token",
                  error_description="The access token expired"
```

A gateway MUST NOT copy a raw status message into that header — it has a grammar
that an arbitrary message will violate. `DebugInfo` MUST be stripped unless
explicitly exposed, which SHOULD be refused on a non-loopback listener.

### 5.4 Gateway-originated errors

Routing, negotiation, binding and validation failures never reach the service.
Same envelope, with `reason` from: `ROUTE_NOT_FOUND`, `METHOD_NOT_ALLOWED`,
`UNSUPPORTED_MEDIA_TYPE`, `NOT_ACCEPTABLE`, `MALFORMED_BODY`, `MALFORMED_PATH`,
`UNKNOWN_QUERY_PARAMETER`, `INVALID_ARGUMENT`, `PAYLOAD_TOO_LARGE`,
`GATEWAY_PANIC`.

A panic MUST be caught and rendered as `500` / `GATEWAY_PANIC`. It MUST NOT drop
the connection — on HTTP/2 and HTTP/3 that connection carries other requests —
and the payload MUST NOT reach the client.

## 6. Streaming

Server streaming only. Client and bidirectional streaming are **not**
transcoded: the generator rejects a `google.api.http` rule on such a method with
a build error rather than emitting a handler that cannot work.

### 6.1 Framings

**JSON array** (default) — one array written incrementally, `[` with the first
message, `,` before each subsequent, `]` at the end. Valid JSON at completion
and parseable by a streaming reader throughout. This is how Google's own REST
endpoints stream.

**SSE** — `event: message` and a `data:` line per message, with a `: keepalive`
comment on an idle interval so intermediaries do not reap the connection.

**Line-delimited** — one compact JSON value per line. What grpc-gateway emits.

**Length-prefixed** — a 4-byte big-endian length per message, matching gRPC's
framing minus the compression flag. The only sensible choice for a binary codec,
since line-delimiting bytes that may contain a newline does not work.

### 6.2 The no-false-2xx rule

**A gateway MUST NOT report a 2xx status for an RPC that did not succeed.**

This is the rule the rest of the protocol is arranged to make satisfiable. Three
cases, and only the third is hard.

**Unary.** The response is fully encoded before the status line is written. The
status is always known in time. No exception.

**Streaming, failure before the first message.** The gateway MUST NOT write the
status line when the stream opens. It defers until the first message or
termination, so a stream failing with `PERMISSION_DENIED` before producing
anything returns a real `403` with a normal error body and no framing at all.
This covers authorization, validation, quota and not-found — the overwhelming
majority of real failures. Deferring costs only the latency of the first
message, which the client is waiting for regardless.

**Streaming, failure after at least one message.** The status line is spent and
no protocol can unspend it. The gateway MUST then do all four:

1. Emit a terminal error frame in-band carrying the §5.1 envelope — `,{…}]` for
   the JSON array, `event: error` for SSE, a final line for line-delimited, a
   final frame for length-prefixed.
2. Set `grpc-status`, `grpc-message` and `grpc-status-details-bin` trailers,
   having advertised `Trailer: grpc-status, grpc-message` in the headers.
3. **Terminate the response body abnormally** — `RST_STREAM` with
   `INTERNAL_ERROR` on HTTP/2 and HTTP/3; on HTTP/1.1, close without the
   terminating zero-length chunk.
4. Record the failure with the full status for the operator, since the client's
   view of it is necessarily degraded.

Step 3 is the one that matters. It means a client reading only the status still
observes a failure: `curl` exits non-zero, `fetch()` rejects, a Go client
returns `io.ErrUnexpectedEOF`. Truncation is the only signal HTTP has left, and
a gateway that closes cleanly instead is lying about the outcome.

A gateway MAY offer `buffer_streams=true`, accumulating the whole stream before
writing anything. It MUST NOT be the default.

`grpc-message` MUST be percent-encoded: a status message routinely holds a
resource name or a quoted value, and a raw newline in a header value is a
request-smuggling vector rather than a formatting nit.

### 6.3 Client disconnect

The gateway MUST cancel the underlying RPC. The resulting `CANCELLED` is logged
at debug and produces no output — there is nobody left to report it to.

## 7. Metadata and deadlines

By default `Foo-Bar` becomes `grpcgateway-foo-bar`, `Grpc-Metadata-Foo` becomes
`foo`, and a `-bin` suffix means base64 binary. Hop-by-hop headers
(`Connection`, `Keep-Alive`, `Transfer-Encoding`, `Upgrade`, `TE`, `Trailer`,
`Proxy-Authenticate`, `Proxy-Authorization`) MUST NOT be forwarded. A gateway
MUST allow this policy to be replaced wholesale, because header handling is
where deployments legitimately differ.

Response metadata comes back as `Grpc-Metadata-` headers, trailers as
`Grpc-Trailer-`.

**Deadlines** come from `Grpc-Timeout`, then a per-method timeout, then the
default; the result is propagated as `grpc-timeout` and expiry is `504`. A
gateway MUST set a default — an unbounded one turns a single slow backend into
connection-pool exhaustion.

`google.api.routing` is projected to `x-goog-request-params`; without it,
AIP-4222 implicit routing sends the primary binding's captures.

## 8. The AIP surface

Recognised so the gateway can document them in OpenAPI and validate them at the
edge:

| AIP | Convention | Behaviour |
| --- | --- | --- |
| 132/158 | `page_size`, `page_token`, `next_page_token` | documented as query params and as the pagination cursor |
| 157 | `read_mask` / `?fields=` | applied to the response before encoding |
| 160 | `filter` | documented with the AIP-160 grammar |
| 161 | `update_mask` | `?updateMask=a.b,c`; drives `IMMUTABLE` validation |
| 154 | `etag` | projected to `ETag`; `If-Match` bound back to the field |
| 151 | `google.longrunning.Operation` | `202` while pending |
| 155 | `request_id` | idempotency deduplication |
| 164 | soft delete | `show_deleted` documented; `undelete` recognised |
| 135 | `force` | required to delete a parent with children |

## 9. OpenAPI v3

3.1 by default, 3.0 on request. Three requirements.

**It must describe failure.** Every operation lists the statuses it can actually
produce — `400`, `401`, `403`, `404`, `409`, `429`, `500`, `503` as applicable —
each `$ref`-ing one shared `Status` schema. A document declaring only `200` and
a `default` propagates the same bug into every generated client.

**Paths must be readable.** `{name=shelves/*/books/*}` expands against the
resource's AIP-123 pattern into `/v1/shelves/{shelf}/books/{book}`, with the
original kept in `x-aip-path-template`.

**It must carry its own navigation.** Postman builds folders from `tags` and
names each request from `summary`; Redoc nests from `x-tagGroups`. Emit neither
and an import is a flat list of raw URLs, which is what grpc-gateway's generator
produces.

| Field | Requirement |
| --- | --- |
| `summary` | MUST be present — an imperative phrase from the AIP pattern and the resource's `singular`/`plural`, e.g. "Get a book" |
| `operationId` | MUST be present and unique: `Service_Method` |
| `tags` | MUST be exactly one — the **resource's** `plural`, title-cased, not the service |
| root `tags` | MUST list every tag with a description, parent-resource-first by AIP-123 depth |
| `servers` | MUST be present, so an importer creates a base-URL variable |
| `securitySchemes` | SHOULD come from `google.api.oauth_scopes` |

The tag is the **resource**, not the service: a `LibraryService` fronting
shelves and books produces `Shelves` and `Books` folders, because the resource
is what a caller navigates. That grouping is derivable only because
`google.api.resource` declares `singular` and `plural`.

---

# Divergences from grpc-gateway

| | grpc-gateway | Here |
| --- | --- | --- |
| Error body | bare `google.rpc.Status`, `code` is the gRPC code | AIP-193 envelope, `code` is the HTTP status |
| Mid-stream failure | error chunk, status stays `200`, clean close | error frame **plus** trailers **plus** abnormal termination |
| Stream header | written on the first message | deferred until the first message or termination |
| Stream envelope | every message wrapped in `{"result": …}` | unwrapped; framing carries the structure |
| `405` | mapped through `UNIMPLEMENTED` to `501` | stays `405`, with `Allow` |
| Unknown query params | silently discarded | `400` naming the field |
| Percent-decoding | mode-dependent, reserved chars kept escaped | segment first, then decode all but `%2F` |
| Validation | none | four sources, before the RPC |
| Codec selection | `Content-Type` only | `?alt=` and `Accept` |
| Template parsing | in the runtime | at build time, compiled |
| Route conflicts | resolved by registration order, silently | a build error naming both and an example |
| Error handling hooks | three, which disagree | one `ErrorRenderer` |

Wire compatibility with grpc-gateway is not a goal. A service can be fronted by
both, but clients will observe these differences.

# Testing

| Layer | Method |
| --- | --- |
| IR | built from the real example protos via `buf build`, so the fixture cannot drift |
| Route compilation | the four template shapes, matched and captured |
| Conflict detection | ambiguous route sets asserted to fail the build |
| Determinism | every target generated twice and byte-compared |
| Cross-target agreement | the Rust and Go tables compared to *each other* — same routes, same scan order, same handler indices, same AIP mutability |
| Protocol | the no-false-2xx rule from both ends, all four framings |
| Transport | a real `h3` client over QUIC, asserted byte-identical to HTTP/1.1 |

The agreement check is the one a per-target golden file cannot make: two golden
files can each be internally consistent while describing different route tables,
and the drift only shows up as two runtimes answering one request differently.

Above all of them sits the conformance run, which starts both runtimes and puts
the same questions to each over a real socket:

```sh
just conformance   # rust vs go, 16 cases plus the transport matrix
just ci            # everything, including the above
```

Neither runtime's own suite can catch a disagreement between them — each is
written against its own behaviour — so this is the only check that the claim on
the first line of this README is true. It found four real defects the first time
it ran; see [Divergences](#divergences-from-grpc-gateway) for what the protocol
requires and `scripts/conformance.sh` for what is asserted.

## CI and releases

| Workflow | Runs on | What it does |
| --- | --- | --- |
| `ci.yaml` | push, pull request | protos (buf + api-linter), Rust (fmt/clippy/test), Go (3 modules), generated-code staleness, and the conformance run |
| `release.yaml` | a `v*` tag | re-verifies against the tag, cross-compiles `protoc-gen-http` for five platforms, publishes a release with checksums and generated notes |
| `dependabot-auto-merge.yaml` | Dependabot pull requests | enables GitHub auto-merge for patch and minor bumps; majors get a comment explaining why they were left |

The release workflow pins every action by commit SHA, because a tag is mutable
and that job signs and publishes. CI pins by tag, where a compromised action
costs a red build rather than a release.

**Auto-merge needs two repository settings**, and the workflow fails loudly
rather than merging if the second is missing:

1. Settings → General → *Allow auto-merge*.
2. A branch protection rule on `main` listing the CI jobs as required checks.

Nothing merges because a workflow decided to. The workflow only queues the pull
request; branch protection holds it until the required checks pass. A workflow
that polled the checks itself would be a second, weaker copy of that rule.

# Non-goals

- **gRPC-Web.** `tonic-web` does it, and it is a different protocol.
- **Wire compatibility with grpc-gateway.** Explicitly abandoned; see above.
- **A general reverse proxy.** Routing is driven by `google.api.http` and
  nothing else.
- **Runtime reflection as a routing source.** Descriptors are read at build
  time. A `prost-reflect` dynamic mode may exist later; it is a separate mode.

# License

Apache-2.0 — see [LICENSE](LICENSE).
