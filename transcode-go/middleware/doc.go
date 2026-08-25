// Package middleware is everything that runs around a call.
//
// # Two planes
//
// The HTTP plane is ordinary net/http middleware over http.Handler —
// compression, TLS identity, anything that needs only the request as bytes. The
// handler being an http.Handler makes that free, and nothing here duplicates
// it.
//
// This package is the message plane: everything that needs the resolved method,
// the bound message, or the typed response. A wrapped http.Handler cannot see
// any of that, because routing has not happened yet when it runs.
//
// [Interceptor] covers the phases that do not need the payload — which is most
// of them, since authn, authz, quota, audit and tracing all key on which method
// was called rather than on what it was sent. Payload access is the
// specialisation: a generated handler that wants it implements [InspectRequest]
// or [InspectResponse] over its own message type.
//
// # Relationship to grpc-gateway's ServeMuxOptions
//
// grpc-gateway's extension model is seventeen option functions, each hooking a
// different point with a different signature, and none able to see the request
// message — which is why it has no validation. Every one has a counterpart
// here:
//
//	WithMiddlewares               → Interceptor, via Stack
//	WithMetadata                  → MetadataAnnotator
//	WithIncomingHeaderMatcher     → Headers.Incoming
//	WithOutgoingHeaderMatcher     → Headers.Outgoing
//	WithOutgoingTrailerMatcher    → Headers.Trailer
//	WithForwardResponseOption     → Interceptor OnResponse
//	WithForwardResponseRewriter   → InspectResponse
//	WithErrorHandler              → one ErrorRenderer
//	WithStreamErrorHandler        → the same ErrorRenderer
//	WithRoutingErrorHandler       → the same ErrorRenderer
//	WithMarshalerOption           → codec.Registry
//	WithHealthEndpointAt          → builtin.Health
//
// Three of those collapse into one. grpc-gateway renders unary errors, stream
// errors and routing errors through separate handlers, and they disagree about
// both status and body shape; here every failure leaves through one renderer,
// so it cannot.
//
// # Relationship to go-grpc-middleware
//
// The builtin set mirrors go-grpc-middleware's, with one deliberate omission:
// retry. There it is a client interceptor, and retrying at the transcoder would be
// wrong — the transcoder cannot know whether a method is idempotent, and replaying
// a non-idempotent one turns a timeout into a duplicate write.
package middleware
