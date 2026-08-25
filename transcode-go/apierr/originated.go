package apierr

// originated.go holds the failures the transcoder raises about the request line
// itself: routing and content negotiation.
//
// Each carries a reason token from the README §5.4 set, so a caller can tell a
// transcoder's rejection from a service's own without parsing prose.

import (
	"fmt"
	"net/http"
	"strings"
)

// RouteNotFound reports that no route matched the request path.
func RouteNotFound(path, domain string) *Error {
	return New(NotFound, fmt.Sprintf("No route matches %q.", path)).
		WithErrorInfo("ROUTE_NOT_FOUND", domain, map[string]string{"path": path})
}

// MethodNotAllowed reports that the path matched but the HTTP method did not.
//
// This stays a 405 and carries Allow. grpc-gateway routes it through
// codes.Unimplemented, which its own status table maps back out as 501, losing
// both the status and the header a client needs to recover.
func MethodNotAllowed(method string, allow []string, domain string) *Error {
	value := strings.Join(allow, ", ")
	return New(Unimplemented, fmt.Sprintf("Method %s is not allowed on this path.", method)).
		WithHTTP(http.StatusMethodNotAllowed).
		WithErrorInfo("METHOD_NOT_ALLOWED", domain, map[string]string{"allow": value}).
		WithHeader("Allow", value)
}

// UnsupportedMediaType reports a Content-Type naming no registered codec.
func UnsupportedMediaType(got string, supported []string, domain string) *Error {
	return New(InvalidArgument, fmt.Sprintf("Content-Type %q is not supported.", got)).
		WithHTTP(http.StatusUnsupportedMediaType).
		WithErrorInfo("UNSUPPORTED_MEDIA_TYPE", domain, map[string]string{
			"supported": strings.Join(supported, ", "),
		})
}

// NotAcceptable reports that nothing in Accept names a registered codec.
//
// A rejection rather than a fallback: answering in a media type the client
// excluded is worse than telling them there is no overlap.
func NotAcceptable(accept string, supported []string, domain string) *Error {
	return New(InvalidArgument, fmt.Sprintf("No supported media type satisfies Accept: %s.", accept)).
		WithHTTP(http.StatusNotAcceptable).
		WithErrorInfo("NOT_ACCEPTABLE", domain, map[string]string{
			"supported": strings.Join(supported, ", "),
		})
}

// PayloadTooLarge reports a request body over the configured limit.
func PayloadTooLarge(limit int64, domain string) *Error {
	return New(InvalidArgument, "Request body is too large.").
		WithHTTP(http.StatusRequestEntityTooLarge).
		WithErrorInfo("PAYLOAD_TOO_LARGE", domain, map[string]string{
			"limit": fmt.Sprintf("%d", limit),
		})
}

// Panicked reports a caught panic.
//
// Rendered as an ordinary 500: the payload never reaches the client, and the
// connection is not dropped, because an unwind in one handler is not a reason to
// fail every in-flight request sharing that connection.
func Panicked(domain, method string) *Error {
	return New(Internal, "Internal error.").
		WithErrorInfo("GATEWAY_PANIC", domain, map[string]string{"method": method})
}
