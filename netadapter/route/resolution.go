package route

// resolution.go is the outcome of resolving one request against the table.

// Outcome is what resolving a request against the table produced.
type Outcome uint8

const (
	// Matched means a route matched: serve it.
	Matched Outcome = iota

	// MethodNotAllowed means the path matched but not for this HTTP method.
	// This is a 405 carrying Allow, and it must not be collapsed into a generic
	// failure: doing that is how grpc-gateway turns a 405 into a 501.
	MethodNotAllowed

	// NotFound means no route matched the path at all.
	NotFound
)

// Resolution is the outcome of resolving a request, with what the caller needs
// to act on it.
type Resolution struct {
	// Outcome is which of the three cases this is.
	Outcome Outcome

	// Route is the winning route, set when Outcome is Matched.
	Route *Route

	// Segments are the raw, still-encoded path segments the route matched.
	// Kept undecoded so [Resolution.Captures] can apply the %2F rule per
	// segment.
	Segments []string

	// Verb is the AIP-136 custom verb, or "".
	Verb string

	// Allow lists the methods bound to this path, for the mandatory Allow
	// header on a 405.
	Allow []string
}

// Captures decodes every capture of a matched route, keyed by protojson name.
//
// It returns the first failure rather than a partial map: a malformed path is a
// 400, and there is nothing useful left to bind.
func (r *Resolution) Captures() (map[string]string, error) {
	if r.Route == nil {
		return nil, nil
	}
	out := make(map[string]string, len(r.Route.Captures))
	for _, capture := range r.Route.Captures {
		value, err := r.Route.Capture(capture, r.Segments)
		if err != nil {
			return nil, err
		}
		out[capture.JSON] = value
	}
	return out, nil
}
