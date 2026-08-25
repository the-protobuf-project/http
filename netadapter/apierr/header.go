package apierr

// header.go projects error details onto HTTP headers.
//
// Several google.rpc details have an HTTP counterpart that a client — or an
// intermediary that never parses the body — can act on. The detail stays in the
// body either way; the header is an additional projection, not a move.

import (
	"fmt"
	"math"
	"net/http"
	"strings"
	"time"
)

// Headers returns the response headers, including those projected from details.
func (e *Error) Headers() http.Header {
	out := http.Header{}
	for name, values := range e.Header {
		for _, value := range values {
			out.Add(name, value)
		}
	}

	for _, detail := range e.Details {
		switch typed := detail.(type) {
		case RetryInfo:
			projectRetryAfter(out, typed.RetryDelay)
		case Help:
			projectHelpLinks(out, typed)
		}
	}

	if e.Code == Unauthenticated && out.Get("WWW-Authenticate") == "" {
		out.Set("WWW-Authenticate", e.challenge())
	}
	return out
}

// challenge builds a well-formed RFC 7235 challenge.
//
// grpc-gateway sets this header to the raw status message, which violates the
// grammar as soon as a message contains a quote — and a message describing a
// rejected token very often does.
func (e *Error) challenge() string {
	realm := "api"
	if info := e.ErrorInfo(); info != nil && info.Domain != "" {
		realm = info.Domain
	}
	// The quotes are written out rather than produced by %q, which would
	// escape the escaping quoteEscape already did.
	return fmt.Sprintf(
		`Bearer realm="%s", error="invalid_token", error_description="%s"`,
		quoteEscape(realm), quoteEscape(e.RenderedMessage()),
	)
}

// projectRetryAfter projects RetryInfo.retry_delay to Retry-After.
//
// The header carries whole seconds, and any fraction rounds up: rounding down
// would invite a retry the server is still not ready for.
func projectRetryAfter(out http.Header, delay time.Duration) {
	if delay <= 0 {
		return
	}
	seconds := int64(math.Ceil(delay.Seconds()))
	out.Set("Retry-After", fmt.Sprintf("%d", seconds))
}

// projectHelpLinks projects each Help.links entry to a Link header.
func projectHelpLinks(out http.Header, help Help) {
	for _, link := range help.Links {
		out.Add("Link", fmt.Sprintf("<%s>; rel=\"help\"", link.URL))
	}
}

// quoteEscape prepares a string for a quoted-string, dropping the control
// characters that cannot appear in a header value at all.
//
// %q would escape them as Go escapes rather than removing them, which produces
// a header value that is syntactically valid and semantically nonsense.
func quoteEscape(s string) string {
	var out strings.Builder
	for _, r := range s {
		switch {
		case r < 0x20 || r == 0x7f:
			continue
		case r == '"' || r == '\\':
			out.WriteByte('\\')
			out.WriteRune(r)
		default:
			out.WriteRune(r)
		}
	}
	return out.String()
}
