package rust

// data.go builds the view a template renders. Every naming and ordering
// decision lives here rather than in a template, so the templates stay
// readable and the decisions stay testable.

// fileData is the whole view for one generated module.
type fileData struct {
	// Banner is the generated-file header.
	Banner string
	// Domain is the API's error domain.
	Domain string
	// Methods are the handler-index-ordered methods.
	Methods []methodData
	// Matches are the deduplicated match sequences.
	Matches []matchData
	// Captures are the deduplicated capture-span sets.
	Captures []captureData
	// Routes is the route table, most specific first.
	Routes []routeData
	// Codecs is the codec registry.
	Codecs []codecData
}

// methodData is one variant of the generated Method enum.
type methodData struct {
	// Variant is the Rust identifier, e.g. GetArtist.
	Variant string
	// FullName is the fully-qualified proto method name.
	FullName string
	// Service is the fully-qualified proto service name.
	Service string
	// Doc is the HTTP binding, rendered as a doc comment.
	Doc string
	// Mutating is whether the method changes state.
	Mutating bool
	// ServerStream is whether the method streams its response.
	ServerStream bool
	// Index is the handler index.
	Index int
}

// matchData is one flattened match sequence.
type matchData struct {
	// Ident is the Rust static name, e.g. M_ARTISTS.
	Ident string
	// Doc describes the shape, e.g. /v1/artists/*.
	Doc string
	// Segments are the Rust Match expressions.
	Segments []string
}

// captureData is one capture-span set.
type captureData struct {
	// Ident is the Rust static name, e.g. CAP_NAME_1_5.
	Ident string
	// Doc describes the template variable it came from.
	Doc string
	// Spans are the Rust Capture expressions.
	Spans []string
}

// routeData is one row of the route table.
type routeData struct {
	// HTTPMethod is the method, e.g. "GET".
	HTTPMethod string
	// MatchIdent names the match sequence.
	MatchIdent string
	// CaptureIdent names the capture set, or NONE.
	CaptureIdent string
	// Verb is the AIP-136 custom verb, or "".
	Verb string
	// Template is the original template text, for diagnostics.
	Template string
	// Variant is the Method enum variant this route dispatches to.
	Variant string
}

// codecData is one entry of the codec registry.
type codecData struct {
	// Name is the ?alt= selector.
	Name string
	// MediaTypes are the types the codec answers to.
	MediaTypes []string
	// Framing is the Rust Framing variant.
	Framing string
	// Index is the registry index.
	Index int
}
