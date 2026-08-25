package netadapter

// adapter.go is the request pipeline: route, negotiate, dispatch, render.

import (
	"net/http"

	"github.com/the-protobuf-project/http/netadapter/apierr"
	"github.com/the-protobuf-project/http/netadapter/codec"
	"github.com/the-protobuf-project/http/netadapter/middleware"
	"github.com/the-protobuf-project/http/netadapter/route"
)

// Dispatcher serves a resolved unary call. Generated code implements it by
// switching on [Call.Handler].
type Dispatcher interface {
	// Dispatch serves one call, returning a rendered reply or a failure.
	//
	// The failure should be a [*apierr.Error]; anything else is rendered as an
	// INTERNAL, because an error the adapter cannot classify is a bug rather
	// than a message to hand the caller.
	Dispatch(call *Call) (*Reply, error)
}

// StreamDispatcher serves a resolved server-streaming call.
//
// Optional: an adapter whose dispatcher does not implement it rejects streaming
// methods with UNIMPLEMENTED rather than serving them wrongly.
type StreamDispatcher interface {
	// DispatchStream serves one streaming call, writing messages through the
	// stream and returning the failure that ended it, if any.
	DispatchStream(call *Call, out *Stream) error
}

// Adapter serves a generated route table over HTTP.
type Adapter struct {
	// table is the compiled route table, already sorted most-specific-first.
	table *route.Table

	// codecs are the codecs this adapter was generated with.
	codecs *codec.Registry

	// dispatch serves resolved calls.
	dispatch Dispatcher

	// domain is the API's error domain, stamped into every ErrorInfo.
	domain string

	// stack is the middleware the adapter runs around a call.
	stack *middleware.Stack

	// selected caches the stack resolved against each method, by handler index.
	//
	// Resolved once, at construction: a selector is a predicate over the method
	// table, and the method table is fixed at generation time. Doing it per
	// request would re-answer a question whose answer cannot change.
	selected []middleware.Selected

	// headers maps names between HTTP and gRPC metadata.
	headers middleware.Headers

	// annotators add metadata to a call from the request.
	annotators []middleware.Annotator

	// options are the tunables a deployment sets.
	options Options
}

// New builds an adapter.
func New(table *route.Table, codecs *codec.Registry, dispatch Dispatcher, domain string, opts ...Option) *Adapter {
	adapter := &Adapter{
		table:    table,
		codecs:   codecs,
		dispatch: dispatch,
		domain:   domain,
		stack:    middleware.NewStack(),
		headers:  middleware.DefaultHeaders(),
		options:  defaultOptions(),
	}
	for _, opt := range opts {
		opt(adapter)
	}
	adapter.resolveStack()
	return adapter
}

// resolveStack resolves the middleware stack against every method.
func (a *Adapter) resolveStack() {
	methods := a.table.Methods()
	a.selected = make([]middleware.Selected, len(methods))
	for i, method := range methods {
		a.selected[i] = a.stack.For(method)
	}
}

// interceptors returns the stack resolved for a handler index.
//
// An index outside the method table cannot happen — newCall rejects it before
// this is reached — but returning an empty selection rather than indexing blind
// keeps a generator bug from becoming a panic on the request path.
func (a *Adapter) interceptors(handler int) middleware.Selected {
	if handler < 0 || handler >= len(a.selected) {
		return middleware.Selected{}
	}
	return a.selected[handler]
}

// Table returns the route table, for a test asserting on resolution directly.
func (a *Adapter) Table() *route.Table { return a.table }

// ServeHTTP implements [net/http.Handler].
//
// Every failure — routing, negotiation, binding, the RPC's own status — leaves
// through [Adapter.renderError], so a 404 and a mid-call PERMISSION_DENIED
// produce the same envelope shape. That single funnel is the structural fix for
// grpc-gateway rendering its three error paths differently.
func (a *Adapter) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	defer a.recoverPanic(w)

	// EscapedPath rather than Path, which net/http has already decoded.
	// Routing on the decoded path would let a %2F invent a segment boundary,
	// which is the one thing README §1.2 step 1 exists to prevent: a two-segment
	// resource name holding a slash would arrive indistinguishable from a
	// genuine three-segment one.
	path := r.URL.EscapedPath()

	resolved := a.table.Resolve(r.Method, path)
	switch resolved.Outcome {
	case route.NotFound:
		a.renderError(w, apierr.RouteNotFound(path, a.domain))
		return
	case route.MethodNotAllowed:
		a.renderError(w, apierr.MethodNotAllowed(r.Method, resolved.Allow, a.domain))
		return
	}

	call, err := a.newCall(r, &resolved)
	if err != nil {
		a.renderError(w, err)
		return
	}
	a.serve(w, call)
}

// serve runs the middleware phases around one resolved call.
//
// The completion phase runs whatever happened, which is why it is deferred:
// every path out of here — a rejection in the route phase, a failed dispatch, a
// truncated stream — is a completed call that logging and metrics must see.
func (a *Adapter) serve(w http.ResponseWriter, call *Call) {
	selected := a.interceptors(call.Handler)
	cx := newCallCx(call.cx, call.started)

	var reply *Reply
	var failure *apierr.Error
	defer func() { a.runComplete(cx, selected, outcomeOf(reply, failure)) }()

	if err := a.runRoute(call.cx, selected); err != nil {
		failure = a.asGatewayError(err)
		errorReply(failure).Write(w)
		return
	}
	if err := runRequest(cx, selected); err != nil {
		failure = a.asGatewayError(err)
		errorReply(failure).Write(w)
		return
	}

	if call.Method.ServerStream {
		failure = a.serveStream(w, call)
		return
	}
	reply, failure = a.serveUnary(w, call, cx, selected)
}

// serveUnary dispatches a unary call and writes its reply.
//
// The response phase runs before anything is written, so a hook can still
// change the status — and a hook that rejects produces an error response rather
// than a half-written success.
func (a *Adapter) serveUnary(w http.ResponseWriter, call *Call, cx *middleware.CallCx, selected middleware.Selected) (*Reply, *apierr.Error) {
	reply, err := a.dispatch.Dispatch(call)
	if err != nil {
		failure := a.asGatewayError(err)
		errorReply(failure).Write(w)
		return nil, failure
	}

	if err := runResponse(cx, selected, reply); err != nil {
		failure := a.asGatewayError(err)
		errorReply(failure).Write(w)
		return nil, failure
	}

	reply.Write(w)
	return reply, nil
}
