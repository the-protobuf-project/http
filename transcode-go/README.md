# transcode

The Go runtime. It transcodes HTTP/JSON onto a gRPC service, executing the route
table `protoc-gen-http` emits.

```go
import "github.com/the-protobuf-project/http/transcode"

handler := transcode.New(routes.NewTable(), routes.NewRegistry(), service, routes.Domain,
    transcode.Use(builtin.NewRecovery(logger)),
    transcode.UseFor(builtin.Bearer(verifier, domain), middleware.Mutating()),
)
http.ListenAndServe(":8080", handler)
```

## Why "transcode" and not "gateway"

Transcoding is what `google.api.http` calls this, and what AIP-127 calls it: a
request arrives over HTTP, is resolved against a table compiled from the
service's own protos, and is handed to a dispatcher as a typed call. Naming the
package after the job means the import path says what the code does rather than
what layer it sits at.

"Gateway" was the alternative and it is worse twice over: it names a topology
rather than a behaviour, and it is the name of the project this one exists to
disagree with.

`transcode.Handler` is an `http.Handler` and nothing more — it owns no listener
and no lifecycle, so it mounts into an existing mux and inherits whatever the
deployment already does about TLS, timeouts and shutdown.

## Layout

| Package | Role |
| --- | --- |
| `transcode` | The pipeline: route, negotiate, bind, dispatch, render. `Handler` is an `http.Handler`. |
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
transcode.UseFor(auth, middleware.Mutating())
transcode.UseFor(quota, middleware.Pattern(route.PatternList))
transcode.UseFor(audit, middleware.Every(
    middleware.Service("music.v1.ArtistService"),
    middleware.Not(middleware.ReadOnly()),
))
```

`Mutating()` resolves against the AIP pattern the generator emitted, so a Create
added to the protos later is covered without this list being touched. A policy
written against a name prefix would silently miss it.

Selection is resolved once per method when the handler is built, not per
request: a selector is a predicate over the method table, and the method table
is fixed at generation time.

`retry` has no counterpart, deliberately. In go-grpc-middleware it is a *client*
interceptor, and retrying here would be wrong: the transcoder cannot know whether
a method is idempotent, and replaying a non-idempotent one turns a timeout into a
duplicate write.

## Known divergence from the Rust runtime

**A syntactically malformed percent-escape never reaches this runtime.**
`net/http` parses the request line with `url.ParseRequestURI`, which rejects
`%zz` before any handler runs, and answers with its own plain-text `400`. The
status is right; the body is not an AIP-193 envelope. No handler can close this,
because no handler runs.

Escapes that are well formed but undecodable — `%FF`, which decodes to invalid
UTF-8 — do reach the transcoder and produce a proper `400` with
`reason: MALFORMED_PATH`, which is what `route.DecodeSegment` is for.

## Testing

```sh
just test-go           # this module
just test-example-go   # the end-to-end example, including both no-false-2xx cases
```
