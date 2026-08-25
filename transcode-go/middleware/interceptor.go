package middleware

// interceptor.go holds the interceptor interfaces.

// Interceptor runs around a call.
//
// The interface itself carries only a name: each phase is a separate optional
// interface, so an implementation names exactly the phases it cares about and
// the stack can skip the ones it does not. That is the Go equivalent of a trait
// with defaulted methods, and it has the useful property that a typo in a hook
// name is a policy that silently never runs — which is why [Stack.Add] checks
// that an interceptor implements at least one.
type Interceptor interface {
	// Name identifies the interceptor in tracing and in a stack diagnostic.
	Name() string
}

// RouteHook runs after routing, before the body is read.
//
// The right place to reject: nothing has been decoded, so a 401 here costs
// nothing. Returning an error skips the call and every later phase except
// [CompleteHook].
type RouteHook interface {
	Interceptor

	// OnRoute inspects a resolved route, rejecting the call by returning an
	// error — typically 401, 403 or 429.
	OnRoute(cx *RouteCx) error
}

// RequestHook runs after the request message is bound and validated, before the
// RPC.
type RequestHook interface {
	Interceptor

	// OnRequest inspects a bound call, rejecting it by returning an error.
	OnRequest(cx *CallCx) error
}

// ResponseHook runs after the RPC returns, before the response is encoded.
//
// This is grpc-gateway's WithForwardResponseOption: the place to set a header
// or change the status from what the handler chose.
type ResponseHook interface {
	Interceptor

	// OnResponse inspects and may rewrite the response parts. Returning an
	// error turns the response into a failure, which is how a response-side
	// policy rejects.
	OnResponse(cx *CallCx, parts *ResponseParts) error
}

// CompleteHook runs after everything, success or failure.
type CompleteHook interface {
	Interceptor

	// OnComplete records how a call ended. It cannot fail and cannot change the
	// response — that has already been written. For logging, metrics and audit.
	OnComplete(cx *CallCx, outcome Outcome)
}

// InspectRequest reads or rewrites a typed request message.
//
// Generic over the message so the generated handler keeps its concrete type:
// this is the opt-in half of the message plane, for the policies that genuinely
// need the payload — redacting a field, enforcing a cross-field invariant,
// stamping a server-side default.
type InspectRequest[M any] interface {
	// InspectRequest inspects or rewrites the bound request, rejecting the call
	// by returning an error.
	InspectRequest(cx *CallCx, message *M) error
}

// InspectResponse reads or rewrites a typed response message.
//
// grpc-gateway's WithForwardResponseRewriter, with the type intact: that hook
// receives a proto.Message and returns any, so a rewriter has to type-switch at
// runtime and can return something the marshaler then fails on.
type InspectResponse[M any] interface {
	// InspectResponse inspects or rewrites the response before encoding.
	InspectResponse(cx *CallCx, message *M) error
}
