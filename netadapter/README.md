# netadapter

The Go runtime. It fronts a gRPC service with an AIP-native HTTP/JSON surface,
executing the route table `protoc-gen-http` emits.

```go
import "github.com/the-protobuf-project/grpc-gateway-rs/netadapter"

adapter := netadapter.New(gateway.NewTable(), gateway.NewRegistry(), service, gateway.Domain,
    netadapter.Use(builtin.NewRecovery(logger)),
    netadapter.UseFor(builtin.Bearer(verifier, domain), middleware.Mutating()),
)
http.ListenAndServe(":8080", adapter)
```

## Why "adapter" and not "gateway"

The name is the design. What this does is adapt one network protocol onto a
service defined in another: a request arrives over HTTP, is resolved against a
table compiled from the service's own protos, and is handed to a dispatcher as a
typed call.

Nothing in that shape is specific to HTTP. The route table, the AIP method
classification, the error model and the phase pipeline are the same machinery an
MCP adapter over the same protos needs. Calling this one "gateway" would have
made the general thing sound like the HTTP special case, and the next adapter
would have had to either copy it or rename it.

What *is* HTTP-specific lives in the packages that say so.

## Layout

| Package | Role |
| --- | --- |
| `netadapter` | The pipeline: route, negotiate, bind, dispatch, render. `Adapter` is an `http.Handler`. |
| `route` | The executor for a compiled table — positional matching, capture spans, the `%2F` rule, and resolution to one of `Matched` / `MethodNotAllowed` / `NotFound`. No parser: templates are compiled by the generator. |
| `codec` | The codec table and content negotiation — `?alt=`, `Accept` with quality values, and the four stream framings. |
| `apierr` | The AIP-193 error model: canonical codes, `google.rpc` details, the envelope, and the header projections (`Retry-After`, `Link`, `WWW-Authenticate`). |
| `stream` | Server streaming and the no-false-2xx rule: the state machine, the framings, the trailers, and the termination a transport must act on. |
| `middleware` | The message plane — `Interceptor` phases, `Selector`, `Stack`, and the header/metadata mapping. |
| `middleware/builtin` | The interceptors that ship: Recovery, Deadline, Auth, RateLimit, RealIP, Validate, Idempotency, Logging, Metrics, Health, CORS. |

## The middleware plane

Mirrors [go-grpc-middleware](https://github.com/grpc-ecosystem/go-grpc-middleware),
with selectors that dispatch on what a method *means*:

```go
netadapter.UseFor(auth, middleware.Mutating())
netadapter.UseFor(quota, middleware.Pattern(route.PatternList))
netadapter.UseFor(audit, middleware.Every(
    middleware.Service("music.v1.ArtistService"),
    middleware.Not(middleware.ReadOnly()),
))
```

`Mutating()` resolves against the AIP pattern the generator emitted, so a Create
added to the protos later is covered without this list being touched. A policy
written against a name prefix would silently miss it.

Selection is resolved once per method when the adapter is built, not per
request: a selector is a predicate over the method table, and the method table
is fixed at generation time.

`retry` has no counterpart, deliberately. In go-grpc-middleware it is a *client*
interceptor, and retrying here would be wrong: the adapter cannot know whether a
method is idempotent, and replaying a non-idempotent one turns a timeout into a
duplicate write.

## Known divergence from the Rust runtime

**A syntactically malformed percent-escape never reaches this runtime.**
`net/http` parses the request line with `url.ParseRequestURI`, which rejects
`%zz` before any handler runs, and answers with its own plain-text `400`. The
status is right; the body is not an AIP-193 envelope. No handler can close this,
because no handler runs.

Escapes that are well formed but undecodable — `%FF`, which decodes to invalid
UTF-8 — do reach the adapter and produce a proper `400` with
`reason: MALFORMED_PATH`, which is what `route.DecodeSegment` is for.

## Testing

```sh
just test-go           # this module
just test-example-go   # the end-to-end example, including both no-false-2xx cases
```
