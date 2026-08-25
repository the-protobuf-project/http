package builtin

// health.go serves a health endpoint.

import (
	"net/http"
)

// ServingStatus is what grpc.health.v1.Health reports.
type ServingStatus uint8

const (
	// Serving means the service is up.
	Serving ServingStatus = iota

	// NotServing means the service is down. Answered as 503, so a load balancer
	// acts on it without reading the body.
	NotServing

	// ServiceUnknown means the service is unknown to the health server, which is
	// 404: reporting 503 would imply it exists and is merely down.
	ServiceUnknown
)

// HTTPStatus is the status this maps to.
func (s ServingStatus) HTTPStatus() int {
	switch s {
	case NotServing:
		return http.StatusServiceUnavailable
	case ServiceUnknown:
		return http.StatusNotFound
	}
	return http.StatusOK
}

// String is the name the health protocol uses.
func (s ServingStatus) String() string {
	switch s {
	case NotServing:
		return "NOT_SERVING"
	case ServiceUnknown:
		return "SERVICE_UNKNOWN"
	}
	return "SERVING"
}

// Checker reports whether a service is serving.
//
// It receives the ?service= parameter, matching the grpc.health.v1.Health/Check
// request field. An empty name asks about the server as a whole.
type Checker func(service string) ServingStatus

// Health serves a health endpoint.
//
// grpc-gateway's WithHealthzEndpoint and WithHealthEndpointAt. It answers before
// routing, so a health check keeps working when the route table cannot serve
// anything else — which is precisely when a health check matters.
//
// It is an http.Handler rather than an interceptor for the same reason: it must
// run before the router, and an interceptor by definition runs after it.
type Health struct {
	// path is the endpoint it answers on.
	path string

	// check reports the status.
	check Checker
}

// Healthz returns a health endpoint at /healthz that always reports serving.
func Healthz() *Health {
	return &Health{path: "/healthz", check: func(string) ServingStatus { return Serving }}
}

// HealthAt returns a health endpoint at a chosen path, backed by a checker.
func HealthAt(path string, check Checker) *Health {
	return &Health{path: path, check: check}
}

// Path is the endpoint this answers on.
func (h *Health) Path() string { return h.path }

// Handles reports whether a request path is this health endpoint.
func (h *Health) Handles(path string) bool { return path == h.path }

// ServeHTTP answers a health check with the same JSON shape
// grpc.health.v1.Health returns over gRPC.
func (h *Health) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	status := h.check(r.URL.Query().Get("service"))

	w.Header().Set("Content-Type", "application/json")
	// Health checks are polled constantly and must never be cached.
	w.Header().Set("Cache-Control", "no-store")
	w.WriteHeader(status.HTTPStatus())

	//nolint:errcheck // A health check the prober cannot read is a failed probe,
	// which is the correct outcome and needs no further reporting.
	_, _ = w.Write([]byte(`{"status":"` + status.String() + `"}`))
}

// Wrap returns a handler that answers health checks and passes everything else
// to next.
func (h *Health) Wrap(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if h.Handles(r.URL.Path) {
			h.ServeHTTP(w, r)
			return
		}
		next.ServeHTTP(w, r)
	})
}
