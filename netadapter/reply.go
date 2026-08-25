package netadapter

// reply.go is a rendered response and how it reaches the wire.

import (
	"net/http"

	"github.com/the-protobuf-project/http/netadapter/apierr"
)

// Reply is a rendered HTTP response.
//
// A handler returns one fully formed rather than writing to a ResponseWriter,
// which is what makes the no-false-2xx rule structural for unary calls: the
// body is already encoded when the status line is written, so an encoding
// failure cannot arrive after a 200 has gone out.
type Reply struct {
	// Status is the status line.
	Status int

	// Header holds the response headers, including any projected from error
	// details.
	Header http.Header

	// Body is the encoded body.
	Body []byte
}

// NewReply returns a reply with a status and a body.
func NewReply(status int, body []byte) *Reply {
	return &Reply{Status: status, Header: http.Header{}, Body: body}
}

// WithHeader sets one response header.
func (r *Reply) WithHeader(name, value string) *Reply {
	if r.Header == nil {
		r.Header = http.Header{}
	}
	r.Header.Set(name, value)
	return r
}

// Write sends the reply.
//
// A 204 gets no body even if one was built: a body on a No Content response is
// a protocol violation, and Go's server would strip it anyway — silently, which
// is worse than doing it here where it can be seen.
func (r *Reply) Write(w http.ResponseWriter) {
	for name, values := range r.Header {
		for _, value := range values {
			w.Header().Add(name, value)
		}
	}
	w.WriteHeader(r.Status)

	if r.Status == http.StatusNoContent || len(r.Body) == 0 {
		return
	}
	//nolint:errcheck // A failed write means the peer is gone; there is nobody
	// left to report it to, and the connection is already being torn down.
	_, _ = w.Write(r.Body)
}

// errorReply renders an error as an AIP-193 response.
//
// The fallback body is a hand-written envelope rather than a bare 500: the only
// way JSON marshalling fails here is a detail carrying an unmarshallable value,
// and a caller who receives an empty body learns nothing at all.
func errorReply(err *apierr.Error) *Reply {
	body, marshalErr := err.JSON()
	if marshalErr != nil {
		body = []byte(`{"error":{"code":500,"message":"Internal error.","status":"INTERNAL"}}`)
		err = apierr.New(apierr.Internal, "Internal error.")
	}

	reply := &Reply{Status: err.HTTP, Header: err.Headers(), Body: body}
	reply.Header.Set("Content-Type", "application/json")
	return reply
}
