package route

// route.go is one compiled binding and the positional walk that matches it.

import (
	"errors"
	"strings"
)

// Route is one compiled google.api.http binding.
//
// A generated table is a package-level slice built once at init and never
// mutated, so a route is copied by value cheaply and shared freely.
type Route struct {
	// Method is the HTTP method this binding answers, e.a. "GET".
	Method string

	// Segments is the flattened match sequence. No element is a variable — the
	// compiler expanded those into their sub-segments.
	Segments []Match

	// Verb is the AIP-136 custom verb without its colon, or "" when the binding
	// declares none.
	Verb string

	// Captures are the capture spans, in template order.
	Captures []Capture

	// Template is the original template text, e.a. "/v1/{name=shelves/*}".
	//
	// Carried for diagnostics, tracing spans and error messages only. It is
	// never parsed; parsing happens once, in the generator.
	Template string

	// Handler indexes the generated method table, which is how a match becomes
	// a call without a map lookup on a string.
	Handler int
}

// HasMulti reports whether the route ends in a "**".
func (r *Route) HasMulti() bool {
	return len(r.Segments) > 0 && r.Segments[len(r.Segments)-1].Kind == KindMulti
}

// fixed is the number of leading segments that must match one-to-one: the full
// length, less the trailing "**" when there is one.
func (r *Route) fixed() int {
	if r.HasMulti() {
		return len(r.Segments) - 1
	}
	return len(r.Segments)
}

// Matches reports whether raw, still-encoded path segments match this route.
//
// The segments must be split on "/" before any percent-decoding, per README
// §1.2: decoding first would let a %2F invent a segment boundary and corrupt an
// AIP-122 resource name.
//
// A failed match costs no allocation, because captures are sliced out
// separately by [Route.Capture] only once a route has won.
func (r *Route) Matches(segments []string, verb string) bool {
	if verb != r.Verb {
		return false
	}

	fixed := r.fixed()
	if r.HasMulti() {
		// "**" matches zero or more, so the path may be shorter than the route
		// is long, but never shorter than the fixed prefix.
		if len(segments) < fixed {
			return false
		}
	} else if len(segments) != fixed {
		return false
	}

	for i, m := range r.Segments[:fixed] {
		switch m.Kind {
		case KindLiteral:
			if segments[i] != m.Literal {
				return false
			}
		case KindSingle:
			// A "*" binds exactly one component, and an empty component — from
			// a doubled or trailing slash — is not one.
			if segments[i] == "" {
				return false
			}
		case KindMulti:
			// Unreachable: "**" cannot appear in the fixed prefix.
			return false
		}
	}
	return true
}

// Capture slices one capture out of a matched path and decodes it.
//
// Decoding happens here, after the match, per README §1.2 step 4: every
// percent-escape is decoded except %2F, which is left as written because "/"
// separates the segments of an AIP-122 resource name.
//
// A "**" binding zero segments yields an empty value, not an error.
func (r *Route) Capture(c Capture, segments []string) (string, error) {
	span := segments[c.Start:c.EndIndex(len(segments))]

	switch len(span) {
	case 0:
		return "", nil
	case 1:
		value, err := DecodeSegment(span[0])
		if err != nil {
			return "", captureErr(c.JSON, err)
		}
		return value, nil
	}

	var out strings.Builder
	for i, segment := range span {
		if i > 0 {
			out.WriteByte('/')
		}
		value, err := DecodeSegment(segment)
		if err != nil {
			return "", captureErr(c.JSON, err)
		}
		out.WriteString(value)
	}
	return out.String(), nil
}

// captureErr attaches the field a decode failure belongs to.
//
// A decode error that is not a [DecodeError] cannot arise from
// [DecodeSegment], but reporting the field is more useful than asserting the
// type and panicking if that ever stops being true.
func captureErr(field string, err error) error {
	kind := ErrBadHex
	var decoded DecodeError
	if errors.As(err, &decoded) {
		kind = decoded
	}
	return &CaptureError{Field: field, Err: kind}
}
