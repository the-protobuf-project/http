package builtin

// cors.go answers browser preflights and stamps CORS headers.

import (
	"fmt"
	"sort"
	"strings"

	"github.com/the-protobuf-project/http/transcode-go/middleware"
)

// defaultMaxAge is how long a browser may cache a preflight, in seconds.
const defaultMaxAge = 600

// CORS answers browser preflights and stamps CORS headers.
//
// Strictly this belongs in the HTTP plane, where any net/http middleware would
// do. It is here because the transcoder knows something a generic layer does not:
// which HTTP methods are actually bound to a path.
// Access-Control-Allow-Methods can therefore be exact rather than a
// hand-maintained list that drifts from the route table.
type CORS struct {
	// origins is the allowlist; nil means any origin.
	origins map[string]bool

	// credentials is whether credentialed requests are allowed.
	credentials bool

	// maxAge is how long a browser may cache a preflight, in seconds.
	maxAge int

	// expose are the response headers made readable to the browser.
	expose []string
}

// PermissiveCORS allows any origin, without credentials.
func PermissiveCORS() *CORS { return &CORS{maxAge: defaultMaxAge} }

// AllowOrigins allows an explicit set of origins, compared exactly.
func AllowOrigins(origins ...string) *CORS {
	allowed := make(map[string]bool, len(origins))
	for _, origin := range origins {
		allowed[origin] = true
	}
	return &CORS{origins: allowed, maxAge: defaultMaxAge}
}

// WithCredentials allows credentialed requests.
//
// It panics when any origin is allowed: the Fetch standard rejects "*" together
// with Access-Control-Allow-Credentials, and a browser refuses the response.
// Failing at construction is better than shipping a configuration that silently
// does not work.
func (c *CORS) WithCredentials() *CORS {
	if c.origins == nil {
		panic("builtin: credentialed CORS requires an explicit origin allowlist")
	}
	c.credentials = true
	return c
}

// Expose makes response headers readable to the browser.
func (c *CORS) Expose(headers ...string) *CORS {
	c.expose = append([]string(nil), headers...)
	sort.Strings(c.expose)
	return c
}

// MaxAge sets how long a browser may cache a preflight, in seconds.
func (c *CORS) MaxAge(seconds int) *CORS {
	c.maxAge = seconds
	return c
}

// Name implements [middleware.Interceptor].
func (*CORS) Name() string { return "cors" }

// OnResponse stamps the CORS headers.
//
// An origin that is not allowed gets no headers rather than a rejection: the
// browser is what enforces this, and a 403 here would confuse a non-browser
// client that sent an Origin for its own reasons.
func (c *CORS) OnResponse(cx *middleware.CallCx, parts *middleware.ResponseParts) error {
	origin := cx.Request.Header.Get("Origin")
	if origin == "" {
		return nil
	}
	allowed, ok := c.allowedOrigin(origin)
	if !ok {
		return nil
	}

	parts.Header.Set("Access-Control-Allow-Origin", allowed)
	if c.credentials {
		parts.Header.Set("Access-Control-Allow-Credentials", "true")
	}
	if len(c.expose) > 0 {
		parts.Header.Set("Access-Control-Expose-Headers", strings.Join(c.expose, ", "))
	}
	if c.origins != nil {
		// An allowlisted response varies by Origin, so a shared cache must not
		// serve one origin's response to another.
		parts.Header.Add("Vary", "Origin")
	}
	parts.Header.Set("Access-Control-Max-Age", fmt.Sprintf("%d", c.maxAge))
	return nil
}

// allowedOrigin is the value to echo for a request's Origin, if it is allowed.
func (c *CORS) allowedOrigin(origin string) (string, bool) {
	if c.origins == nil {
		return "*", true
	}
	if c.origins[origin] {
		return origin, true
	}
	return "", false
}
