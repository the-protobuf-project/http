package table

// ident.go derives the words a generated identifier is built from, and the keys
// that decide when two routes can share one.
//
// Words rather than identifiers: SCREAMING_SNAKE is right in Rust and Python
// and wrong in Go, so the casing belongs to the target. What must not differ is
// which routes are considered the same, which is what the keys below decide.

import (
	"fmt"
	"strings"

	"github.com/the-protobuf-project/protokit/service/httprule"
)

// MatchKey identifies a match sequence by its shape, so two bindings that walk
// the same path share one emitted sequence.
//
// Several bindings usually do: GET, PATCH and DELETE on a resource all walk
// `/v1/artists/*`, and emitting three identical arrays would triple the table
// for nothing.
func MatchKey(route *httprule.Route) string {
	parts := make([]string, len(route.Segments))
	for i, segment := range route.Segments {
		switch segment.Kind {
		case httprule.KindLiteral:
			parts[i] = "L:" + segment.Literal
		case httprule.KindSingle:
			parts[i] = "*"
		case httprule.KindMulti:
			parts[i] = "**"
		}
	}
	return strings.Join(parts, "/")
}

// CaptureKey identifies a capture set by its field paths and spans.
//
// Two routes share a capture set only when they bind the same fields at the
// same positions: `{name=artists/*}` and `{name=artists/*/tracks/*}` both bind
// "name" but slice different spans out of the path.
func CaptureKey(route *httprule.Route) string {
	parts := make([]string, len(route.Captures))
	for i, capture := range route.Captures {
		parts[i] = fmt.Sprintf("%s:%d:%d", capture.Name(), capture.Start, capture.End)
	}
	return strings.Join(parts, ",")
}

// MatchWords returns the words a match sequence is named after: its path shape,
// with the wildcards spelled out.
//
// Spelling them matters. `/v1/artists` and `/v1/artists/*` pin the same
// literal, so naming by literals alone would collide and leave a reader with
// two identifiers distinguished by a numeric suffix, neither of which says
// which is which. Spelling the shape gives "artists" and "artists any".
//
// The version segment is dropped because every route in a v1 API pins "v1", so
// it distinguishes nothing.
func MatchWords(route *httprule.Route) []string {
	var words []string
	for _, segment := range route.Segments {
		switch segment.Kind {
		case httprule.KindLiteral:
			if !isVersion(segment.Literal) {
				words = append(words, segment.Literal)
			}
		case httprule.KindSingle:
			words = append(words, "any")
		case httprule.KindMulti:
			words = append(words, "rest")
		}
	}
	if len(words) == 0 {
		words = append(words, "root")
	}
	return words
}

// CaptureWords returns the words a capture set is named after: the fields it
// binds, with the path separators flattened.
func CaptureWords(route *httprule.Route) []string {
	words := make([]string, len(route.Captures))
	for i, capture := range route.Captures {
		words[i] = strings.ReplaceAll(capture.Name(), ".", "_")
	}
	return words
}

// SpanWords returns the first capture's span as words, for a target that puts
// the span in the identifier.
//
// A span disambiguates what the field name alone cannot: two routes both
// binding "name" from different positions are different sets, and a name that
// omitted the span would collide and be resolved by a numeric suffix that says
// nothing.
func SpanWords(route *httprule.Route) []string {
	if len(route.Captures) == 0 {
		return nil
	}
	span := route.Captures[0]
	end := "end"
	if span.End != httprule.ToEnd {
		end = fmt.Sprintf("%d", span.End)
	}
	return []string{fmt.Sprintf("%d", span.Start), end}
}

// Shape renders a route's match sequence as a path, for a doc comment.
func Shape(route *httprule.Route) string {
	parts := make([]string, 0, len(route.Segments))
	for _, segment := range route.Segments {
		switch segment.Kind {
		case httprule.KindLiteral:
			parts = append(parts, segment.Literal)
		case httprule.KindSingle:
			parts = append(parts, "*")
		case httprule.KindMulti:
			parts = append(parts, "**")
		}
	}
	return "/" + strings.Join(parts, "/")
}

// CaptureDoc describes what a capture set binds, for a doc comment.
func CaptureDoc(route *httprule.Route) string {
	names := make([]string, len(route.Captures))
	for i, capture := range route.Captures {
		names[i] = capture.Name()
	}
	return strings.Join(names, ", ") + " in " + route.Template.Raw
}

// isVersion reports whether a literal is a version segment, which carries no
// information in an identifier: every route in a v1 API pins "v1".
func isVersion(literal string) bool {
	if len(literal) < 2 || literal[0] != 'v' {
		return false
	}
	for _, c := range literal[1:] {
		if c < '0' || c > '9' {
			return false
		}
	}
	return true
}
