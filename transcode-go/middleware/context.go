package middleware

// context.go is what an interceptor sees.

import (
	"context"
	"net/http"
	"time"

	"github.com/the-protobuf-project/http/transcode-go/apierr"
	"github.com/the-protobuf-project/http/transcode-go/route"
)

// RouteCx is what is known once a route has matched, before the body is read.
//
// This is the phase most policies run in: authn, authz, quota and audit all key
// on which method was reached, not on what it was sent.
type RouteCx struct {
	// Ctx carries values between interceptors and on into the handler. An
	// interceptor that authenticates a caller puts the identity here and an
	// authorizer reads it, without either knowing about the other.
	//
	// Replacing it is how a value is added: context.WithValue returns a new
	// context, and assigning it back is what makes it visible downstream.
	Ctx context.Context

	// Request is the request as received.
	Request *http.Request

	// Method is the resolved method, from the generated method table.
	Method route.Method

	// Template is the matched path template, e.a. "/v1/{name=artists/*}".
	//
	// grpc-gateway exposes this as HTTPPathPattern(ctx). It is the right label
	// for a metric, because it has bounded cardinality where the concrete path
	// does not.
	Template string

	// Captures are the path captures, keyed by protojson field path.
	Captures map[string]string

	// Peer is the client address the transport reported, which is the proxy's
	// address behind one. Use builtin.RealIP to recover the original.
	Peer string

	// Metadata is what will be forwarded to the service, which an annotator may
	// extend.
	Metadata Metadata
}

// Set stores a value on the context, for one interceptor to pass to another.
func (c *RouteCx) Set(key, value any) { c.Ctx = context.WithValue(c.Ctx, key, value) }

// CallCx is a call in progress: everything in [RouteCx] plus timing and the
// deadline.
type CallCx struct {
	// RouteCx is the routing context, embedded so a hook reaches the method and
	// the template without a level of indirection.
	*RouteCx

	// Started is when the call began, for latency measurement.
	Started time.Time

	// Deadline is the call's budget, from Grpc-Timeout or configuration. Zero
	// means none was set.
	Deadline time.Duration
}

// Elapsed is how long the call has been running.
func (c *CallCx) Elapsed() time.Duration { return time.Since(c.Started) }

// Expired reports whether the deadline has passed.
func (c *CallCx) Expired() bool { return c.Deadline > 0 && c.Elapsed() >= c.Deadline }

// DeadlineError is the DEADLINE_EXCEEDED failure for an expired call.
func (c *CallCx) DeadlineError(domain string) *apierr.Error {
	return apierr.New(apierr.DeadlineExceeded,
		"The deadline expired before the operation could complete.").
		WithErrorInfo("DEADLINE_EXCEEDED", domain, map[string]string{
			"method": c.Method.FullName,
		})
}

// ResponseParts is the response, before it is written.
//
// This is what a [ResponseHook] mutates, and it is the counterpart of
// grpc-gateway's WithForwardResponseOption.
type ResponseParts struct {
	// Status is the status line. A hook may change it — an AIP-133 Create
	// promoting 200 to 201, for instance.
	Status int

	// Header holds the response headers.
	Header http.Header

	// Trailer holds the trailers, emitted when the client asked for them.
	Trailer http.Header
}

// NewResponseParts returns an empty 200.
func NewResponseParts() *ResponseParts {
	return &ResponseParts{
		Status:  http.StatusOK,
		Header:  http.Header{},
		Trailer: http.Header{},
	}
}

// Outcome is how a call ended, for [CompleteHook].
type Outcome struct {
	// Status is the HTTP status the client saw.
	Status int

	// Err is the failure, or nil on success.
	Err *apierr.Error
}

// Code is the canonical code name, "OK" on success.
func (o Outcome) Code() string {
	if o.Err == nil {
		return apierr.OK.String()
	}
	return o.Err.Code.String()
}

// Failed reports whether the call failed.
func (o Outcome) Failed() bool { return o.Err != nil }
