package stream

// trailer.go builds the gRPC trailers a stream ends with.

import (
	"fmt"
	"net/http"
	"strings"

	"github.com/the-protobuf-project/grpc-gateway-rs/netadapter/apierr"
)

// Advertised is the value of the Trailer response header.
//
// Trailers must be advertised in the headers for an intermediary to be obliged
// to preserve them, and Go's HTTP server requires the same declaration before it
// will send them at all.
const Advertised = "grpc-status, grpc-message"

// Trailers are the gRPC trailers a stream ends with.
//
// Trailers are how gRPC reports an outcome that was not known when the headers
// went out, which is exactly the streaming case. They only reach a client that
// asked for them, which is why they are never the only signal — see
// [TerminationTruncate].
type Trailers struct {
	// Status is grpc-status: the canonical code as a number.
	Status int32

	// Message is grpc-message: the human-readable message, percent-encoded.
	Message string

	// Details is grpc-status-details-bin: base64 google.rpc.Status, when there
	// are details worth carrying.
	Details string
}

// OKTrailers returns the trailers for a stream that completed.
func OKTrailers() Trailers { return Trailers{Status: int32(apierr.OK)} }

// ErrorTrailers returns the trailers for a stream that failed.
func ErrorTrailers(err *apierr.Error) Trailers {
	return Trailers{
		Status:  int32(err.Code),
		Message: percentEncode(err.Message),
	}
}

// Apply writes the trailers onto a response header map.
//
// Go's HTTP server sends a header as a trailer when its name is prefixed with
// http.TrailerPrefix, which is what makes trailers reachable from an ordinary
// handler without knowing which HTTP version is underneath.
func (t Trailers) Apply(header http.Header) {
	header.Set(http.TrailerPrefix+"grpc-status", fmt.Sprintf("%d", t.Status))
	if t.Message != "" {
		header.Set(http.TrailerPrefix+"grpc-message", t.Message)
	}
	if t.Details != "" {
		header.Set(http.TrailerPrefix+"grpc-status-details-bin", t.Details)
	}
}

// percentEncode encodes a grpc-message, which must be ASCII.
//
// A status message routinely contains a resource name or a quoted value, and an
// un-encoded newline or non-ASCII byte in a header value is a request-smuggling
// vector rather than merely a formatting problem.
func percentEncode(message string) string {
	var out strings.Builder
	for i := 0; i < len(message); i++ {
		b := message[i]
		if (b > 0x20 && b < 0x7f && b != '%') || b == ' ' {
			out.WriteByte(b)
			continue
		}
		fmt.Fprintf(&out, "%%%02X", b)
	}
	return out.String()
}
