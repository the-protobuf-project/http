package route

// decode.go percent-decodes captured path segments.

import (
	"fmt"
	"strings"
	"unicode/utf8"
)

// DecodeError says why a captured segment could not be decoded.
//
// Each is a 400 with INVALID_ARGUMENT and reason MALFORMED_PATH — never a 404.
// The path matched; the value is what is wrong.
type DecodeError uint8

const (
	// ErrTruncated is a "%" with fewer than two characters after it.
	ErrTruncated DecodeError = iota

	// ErrBadHex is a "%" followed by something that is not two hex digits.
	ErrBadHex

	// ErrNotUTF8 is a segment whose decoded bytes are not valid UTF-8.
	ErrNotUTF8
)

// Description is a short explanation, suitable for a FieldViolation.
func (e DecodeError) Description() string {
	switch e {
	case ErrTruncated:
		return "truncated percent-escape"
	case ErrBadHex:
		return "percent-escape is not two hex digits"
	case ErrNotUTF8:
		return "decodes to invalid UTF-8"
	}
	return "malformed path segment"
}

// Error implements error.
func (e DecodeError) Error() string { return e.Description() }

// CaptureError is a decode failure carrying the field it belongs to.
//
// The field travels with the error so the caller can raise a FieldViolation
// naming what the client actually sent, rather than a bare "malformed path".
type CaptureError struct {
	// Field is the protojson field path, e.a. "book.name".
	Field string

	// Err is what was wrong with the encoding.
	Err DecodeError
}

// Error implements error.
func (e *CaptureError) Error() string {
	return fmt.Sprintf("%s: %s", e.Field, e.Err.Description())
}

// Unwrap exposes the underlying [DecodeError] to errors.Is and errors.As.
func (e *CaptureError) Unwrap() error { return e.Err }

// DecodeSegment percent-decodes one path segment, preserving %2F.
//
// That exception is the rule, not a detail. "/" separates the segments of an
// AIP-122 resource name, so decoding %2F would make a captured name ambiguous
// with a genuinely longer one: shelves/a%2Fb and shelves/a/b would arrive
// identical, and nothing downstream could tell a two-segment name holding a
// slash from a three-segment name.
//
// Every other escape decodes, including multi-byte UTF-8, which is
// percent-encoded one byte at a time.
//
// The common case — a resource id with no escapes at all — returns the input
// unchanged and allocates nothing.
func DecodeSegment(segment string) (string, error) {
	if !strings.Contains(segment, "%") {
		return segment, nil
	}

	out := make([]byte, 0, len(segment))
	for i := 0; i < len(segment); {
		if segment[i] != '%' {
			out = append(out, segment[i])
			i++
			continue
		}
		if i+2 >= len(segment) {
			return "", ErrTruncated
		}
		hi, ok := hexVal(segment[i+1])
		if !ok {
			return "", ErrBadHex
		}
		lo, ok := hexVal(segment[i+2])
		if !ok {
			return "", ErrBadHex
		}

		if b := hi<<4 | lo; b == '/' {
			// Left encoded on purpose; see this function's documentation.
			out = append(out, segment[i:i+3]...)
		} else {
			out = append(out, b)
		}
		i += 3
	}

	if !utf8.Valid(out) {
		return "", ErrNotUTF8
	}
	return string(out), nil
}

// hexVal decodes one hex digit, reporting whether it was one.
func hexVal(b byte) (byte, bool) {
	switch {
	case b >= '0' && b <= '9':
		return b - '0', true
	case b >= 'a' && b <= 'f':
		return b - 'a' + 10, true
	case b >= 'A' && b <= 'F':
		return b - 'A' + 10, true
	}
	return 0, false
}
