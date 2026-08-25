package golang

// data.go is the view a template renders.

// fileData is the whole view for one generated package.
type fileData struct {
	// Banner is the generated-file header.
	Banner string
	// Package is the Go package name.
	Package string
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

// methodData is one entry of the generated method table.
type methodData struct {
	// Const is the Go constant naming the handler index, e.g. MethodGetArtist.
	Const string
	// FullName is the fully-qualified proto method name.
	FullName string
	// Service is the fully-qualified proto service name.
	Service string
	// Name is the RPC's own name.
	Name string
	// Doc is the HTTP binding, rendered as a doc comment.
	Doc string
	// Pattern is the route.Pattern constant, e.g. "PatternGet".
	Pattern string
	// Mutating is whether the method changes state.
	Mutating bool
	// ServerStream is whether the method streams its response.
	ServerStream bool
	// Index is the handler index.
	Index int
}

// matchData is one flattened match sequence.
type matchData struct {
	// Ident is the Go variable name, e.g. matchArtistsAny.
	Ident string
	// Doc describes the shape, e.g. /v1/artists/*.
	Doc string
	// Segments are the Go route.Match expressions.
	Segments []string
}

// captureData is one capture-span set.
type captureData struct {
	// Ident is the Go variable name, e.g. captureName1To5.
	Ident string
	// Doc describes the template variable it came from.
	Doc string
	// Spans are the Go route.Capture expressions.
	Spans []string
}

// routeData is one row of the route table.
type routeData struct {
	// HTTPMethod is the method, e.g. "GET".
	HTTPMethod string
	// MatchIdent names the match sequence.
	MatchIdent string
	// CaptureIdent names the capture set, or the empty set.
	CaptureIdent string
	// Verb is the AIP-136 custom verb, or "".
	Verb string
	// Template is the original template text, for diagnostics.
	Template string
	// Handler is the method constant this route dispatches to.
	Handler string
}

// codecData is one entry of the codec registry.
type codecData struct {
	// Name is the ?alt= selector.
	Name string
	// MediaTypes are the types the codec answers to.
	MediaTypes []string
	// Framing is the Go codec.Framing constant.
	Framing string
	// Index is the registry index.
	Index int
}
