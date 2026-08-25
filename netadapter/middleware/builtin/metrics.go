package builtin

// metrics.go reports every call to a sink.

import (
	"time"

	"github.com/the-protobuf-project/grpc-gateway-rs/netadapter/middleware"
)

// RequestMetric is one completed call.
//
// Every field is bounded-cardinality on purpose: the template rather than the
// concrete path, and the code rather than the error message. A label that can
// take unbounded values is how a metrics backend gets taken down by the service
// it was meant to observe.
type RequestMetric struct {
	// Service is the fully-qualified service name.
	Service string

	// Method is the fully-qualified method name.
	Method string

	// Template is the path template, e.a. "/v1/{name=artists/*}".
	Template string

	// HTTPMethod is the request method.
	HTTPMethod string

	// Status is the response status.
	Status int

	// Code is the canonical code name, "OK" on success.
	Code string

	// Latency is how long the call took.
	Latency time.Duration
}

// MetricsSink receives metrics.
//
// An interface rather than a Prometheus dependency: which client a deployment
// uses, and whether it uses Prometheus at all, is not this module's business.
// go-grpc-middleware makes the same split with providers/prometheus.
type MetricsSink interface {
	// Record records one completed call.
	Record(metric RequestMetric)
}

// Metrics reports every call to a sink.
type Metrics struct {
	// sink receives the metrics.
	sink MetricsSink
}

// NewMetrics returns the interceptor.
func NewMetrics(sink MetricsSink) *Metrics { return &Metrics{sink: sink} }

// Name implements [middleware.Interceptor].
func (*Metrics) Name() string { return "metrics" }

// OnComplete implements [middleware.CompleteHook].
func (m *Metrics) OnComplete(cx *middleware.CallCx, outcome middleware.Outcome) {
	m.sink.Record(RequestMetric{
		Service:    cx.Method.Service,
		Method:     cx.Method.FullName,
		Template:   cx.Template,
		HTTPMethod: cx.Request.Method,
		Status:     outcome.Status,
		Code:       outcome.Code(),
		Latency:    cx.Elapsed(),
	})
}
