package route

// segment.go holds the two value types a compiled template is made of.

// Kind is what one element of a compiled template matches.
type Kind uint8

const (
	// KindLiteral is a fixed path component, compared byte-exact against the
	// still-encoded request segment.
	KindLiteral Kind = iota

	// KindSingle is "*": exactly one non-empty component.
	KindSingle

	// KindMulti is "**": zero or more components. Only ever the final element
	// of a route.
	KindMulti
)

// Match is one element of a compiled template.
//
// There is no variable kind: the compiler expands variables into their
// sub-segments and records the spans in [Route.Captures], which is what turns
// matching into a flat positional walk.
type Match struct {
	// Kind is what this element matches.
	Kind Kind

	// Literal is the component to compare, set only when Kind is KindLiteral.
	Literal string
}

// Literal returns a literal match, for a generated table to read cleanly.
func Literal(s string) Match { return Match{Kind: KindLiteral, Literal: s} }

// Single returns a "*" match.
func Single() Match { return Match{Kind: KindSingle} }

// Multi returns a "**" match.
func Multi() Match { return Match{Kind: KindMulti} }

// Rank orders segment kinds from most to least specific: a literal outranks a
// "*", which outranks a "**".
//
// This mirrors the generator's precedence, which is where it is actually
// applied — a route table arrives already sorted. It exists here for assertions
// and diagnostics, not for the matching path.
func (m Match) Rank() uint8 { return uint8(m.Kind) }

// Wildcard reports whether this element matches any component regardless of its
// content.
func (m Match) Wildcard() bool { return m.Kind == KindSingle || m.Kind == KindMulti }

// ToEnd marks a [Capture] span that runs to the end of the path.
//
// Spans are integers rather than an optional index so the generated table is a
// plain array of numbers in every target language.
const ToEnd = -1

// Capture is where one template variable's value lives in a matched path.
//
// Indices count from the start of the path, which is well defined precisely
// because "**" may only appear last: every span except one ending in "**" sits
// at a fixed position no matter how long the request path turns out to be.
type Capture struct {
	// Field is the request-message field path this span binds, in proto field
	// names: {book.name=*} yields ["book", "name"]. The generator emits typed
	// setters against this, so the runtime never resolves it.
	Field []string

	// JSON is the protojson spelling of Field, e.a. "book.displayName". This is
	// the name a BadRequest.FieldViolation reports and the name OpenAPI
	// documents, so a caller sees one spelling everywhere.
	JSON string

	// Start is the first segment index of the span, inclusive.
	Start int

	// End is one past the last segment index, or [ToEnd] when the span ends in
	// a "**" and so extends to the path's final segment.
	End int
}

// EndIndex resolves the span's exclusive end against a concrete path length.
func (c Capture) EndIndex(pathLen int) int {
	if c.End == ToEnd {
		return pathLen
	}
	return c.End
}
