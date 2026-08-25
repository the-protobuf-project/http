package netadapter

// options.go holds the tunables a deployment sets.

import (
	"log/slog"

	"github.com/the-protobuf-project/grpc-gateway-rs/netadapter/middleware"
)

// defaultMaxRequestBody is the body limit an adapter starts with: 4 MiB, which
// is gRPC's own default maximum message size.
//
// A limit rather than none, because an unbounded body is a way to exhaust a
// server's memory with one request, and an adapter is by definition reachable
// from outside.
const defaultMaxRequestBody = 4 << 20

// Options are the tunables a deployment sets.
type Options struct {
	// MaxRequestBody is the largest request body accepted, in bytes. A larger
	// one is rejected with 413 before any decoding happens.
	MaxRequestBody int64

	// ExposeDebugInfo keeps google.rpc.DebugInfo details in error responses.
	//
	// Off by default: a DebugInfo describes the shape of the service, and a
	// deployment that turns it on should be one an operator is looking at
	// directly.
	ExposeDebugInfo bool

	// Logger records failures the client's view of cannot show — a panic, and a
	// stream that failed after committing its status.
	Logger *slog.Logger
}

// Option configures a adapter.
//
// It takes the adapter rather than the Options because several options
// configure something other than a tunable — the middleware stack, the header
// matchers — and a second mechanism for those would just be two lists to keep
// in step.
type Option func(*Adapter)

// defaultOptions returns the options an adapter starts with.
func defaultOptions() Options {
	return Options{
		MaxRequestBody: defaultMaxRequestBody,
		Logger:         slog.Default(),
	}
}

// WithMaxRequestBody sets the largest request body accepted.
func WithMaxRequestBody(bytes int64) Option {
	return func(a *Adapter) { a.options.MaxRequestBody = bytes }
}

// WithDebugInfo keeps DebugInfo details in error responses.
//
// It takes an explicit bool rather than being a bare toggle so a deployment can
// wire it to a configuration flag without a conditional around the option list.
func WithDebugInfo(expose bool) Option {
	return func(a *Adapter) { a.options.ExposeDebugInfo = expose }
}

// WithLogger sets the logger failures are recorded to.
func WithLogger(logger *slog.Logger) Option {
	return func(a *Adapter) { a.options.Logger = logger }
}

// Use registers an interceptor for every method.
func Use(interceptor middleware.Interceptor) Option {
	return func(a *Adapter) { a.stack.Use(interceptor) }
}

// UseFor registers an interceptor for the methods a selector matches.
//
// This is where the AIP classification pays off: UseFor(auth,
// middleware.Mutating()) covers every Create, Update, Delete and Undelete, and
// keeps covering them when one is added later. A policy written against a name
// prefix would silently miss it.
func UseFor(interceptor middleware.Interceptor, selector middleware.Selector) Option {
	return func(a *Adapter) { a.stack.UseFor(interceptor, selector) }
}

// WithHeaders replaces the header/metadata matchers, which is grpc-gateway's
// WithIncomingHeaderMatcher, WithOutgoingHeaderMatcher and
// WithOutgoingTrailerMatcher in one place.
func WithHeaders(headers middleware.Headers) Option {
	return func(a *Adapter) { a.headers = headers }
}

// WithAnnotator registers a metadata annotator, which is grpc-gateway's
// WithMetadata. Several may be registered; each sees what the ones before it
// added.
func WithAnnotator(annotator middleware.Annotator) Option {
	return func(a *Adapter) { a.annotators = append(a.annotators, annotator) }
}
