package netadapter

// call.go is the resolved request a handler receives.

import (
	"context"
	"net/http"
	"time"

	"github.com/the-protobuf-project/grpc-gateway-rs/netadapter/apierr"
	"github.com/the-protobuf-project/grpc-gateway-rs/netadapter/codec"
	"github.com/the-protobuf-project/grpc-gateway-rs/netadapter/middleware"
	"github.com/the-protobuf-project/grpc-gateway-rs/netadapter/route"
)

// Call is a resolved request, ready for a method handler.
//
// Captures are already decoded; the body is still bytes, because which codec
// decodes it was decided by negotiation and the handler knows its own types.
type Call struct {
	// cx is the middleware context for this call, kept unexported because a
	// handler reaches it through [Call.Context] and the phases reach it
	// directly. Exposing the struct would invite a handler to mutate what the
	// route phase already decided.
	cx *middleware.RouteCx

	// started is when the call began, for latency measurement.
	started time.Time

	// Request is the underlying request, for a handler that needs the context,
	// the headers, or the peer address.
	Request *http.Request

	// Method is the method being served, from the generated method table.
	Method route.Method

	// Handler is the method's index in that table, which is what a generated
	// dispatch switches on.
	Handler int

	// Route is the binding that matched, for diagnostics and tracing.
	Route *route.Route

	// Path holds the path captures, keyed by protojson field path.
	Path map[string]string

	// Query holds the query parameters, with the system parameters removed.
	Query Query

	// Body is the raw request body, empty when the binding accepts none.
	Body []byte

	// RequestCodec is the codec the body decodes with, nil when there is no
	// body.
	RequestCodec *codec.Entry

	// ResponseCodec is the negotiated response codec, always set.
	ResponseCodec *codec.Entry

	// Domain is the API's error domain, so a handler raising an error does not
	// have to thread it separately.
	Domain string
}

// Context returns the call's context, carrying whatever the interceptors put
// there — an authenticated identity, a resolved client address, a trace span.
//
// This rather than Request.Context(): an interceptor that adds a value replaces
// the context on the middleware side, and the original request never sees it.
func (c *Call) Context() context.Context { return c.cx.Ctx }

// Metadata is what will be forwarded to the service, as the incoming matcher
// and any annotators built it.
func (c *Call) Metadata() middleware.Metadata { return c.cx.Metadata }

// Capture returns a required path capture.
//
// A missing capture means the route table and the handler disagree, which is a
// generator bug rather than a caller error — so it is a 500, not a 400.
func (c *Call) Capture(field string) (string, error) {
	if value, ok := c.Path[field]; ok {
		return value, nil
	}
	return "", apierr.BindingMismatch(field, c.Domain, c.Method.FullName)
}

// RejectUnknownQuery returns an error naming every query parameter that is not
// in known.
//
// The bindable set is a property of the binding, which the generated handler
// knows and the runtime does not, so the check is offered here rather than
// performed automatically. Rejecting is the point: grpc-gateway discards
// unknown parameters, which turns a typo in an update call into a silent no-op.
func (c *Call) RejectUnknownQuery(known ...string) error {
	unknown := c.Query.Unknown(known)
	if len(unknown) == 0 {
		return nil
	}
	return apierr.UnknownQueryParameter(unknown, c.Domain, c.Method.FullName)
}

// Invalid returns a 400 naming the fields that failed validation.
func (c *Call) Invalid(violations ...apierr.FieldViolation) error {
	return apierr.InvalidFields(violations, "INVALID_ARGUMENT", c.Domain, c.Method.FullName)
}

// Errorf returns an error with the given code, carrying this call's domain and
// method in its ErrorInfo.
func (c *Call) Errorf(code apierr.Code, message string) error {
	return apierr.New(code, message).
		WithErrorInfo(code.String(), c.Domain, map[string]string{"method": c.Method.FullName})
}
