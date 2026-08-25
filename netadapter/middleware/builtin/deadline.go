package builtin

// deadline.go bounds how long a call may run.

import (
	"fmt"
	"time"

	"github.com/the-protobuf-project/grpc-gateway-rs/netadapter/middleware"
)

// defaultMaxDeadline is the ceiling a client may request.
const defaultMaxDeadline = 5 * time.Minute

// Deadline bounds how long a call may run.
//
// The deadline comes from, in order: the client's Grpc-Timeout header, then the
// configured default. An adapter must always have one — an unbounded default
// turns a single slow backend into connection-pool exhaustion, and by the time
// that is visible the cause is several layers away.
//
// A client asking for longer than the maximum is capped rather than refused,
// since the request is otherwise perfectly valid.
type Deadline struct {
	// fallback is the deadline used when the client asks for none.
	fallback time.Duration

	// max is the ceiling a client may request.
	max time.Duration

	// domain is the API's error domain.
	domain string
}

// NewDeadline returns a deadline with the given default and a five-minute
// ceiling.
func NewDeadline(fallback time.Duration, domain string) *Deadline {
	return &Deadline{fallback: fallback, max: defaultMaxDeadline, domain: domain}
}

// WithMax sets the ceiling a client may request.
func (d *Deadline) WithMax(max time.Duration) *Deadline {
	d.max = max
	return d
}

// Resolve returns the deadline for one request.
func (d *Deadline) Resolve(cx *middleware.RouteCx) time.Duration {
	requested, ok := middleware.ParseGrpcTimeout(cx.Request.Header.Get("Grpc-Timeout"))
	if !ok {
		return d.fallback
	}
	return min(requested, d.max)
}

// Name implements [middleware.Interceptor].
func (*Deadline) Name() string { return "deadline" }

// OnRoute forwards the resolved deadline to the service as grpc-timeout, so the
// backend stops working on a call the adapter has already abandoned.
func (d *Deadline) OnRoute(cx *middleware.RouteCx) error {
	deadline := d.Resolve(cx)
	cx.Metadata.Append("grpc-timeout", fmt.Sprintf("%dm", deadline.Milliseconds()))
	return nil
}

// OnRequest fails the call if the deadline passed while it was in flight.
func (d *Deadline) OnRequest(cx *middleware.CallCx) error {
	if cx.Expired() {
		return cx.DeadlineError(d.domain)
	}
	return nil
}
