package route

// method.go describes the methods a route table dispatches to.

// Method describes one method a route can dispatch to.
//
// It is what a matched route resolves to, and it is deliberately small: a
// runtime needs the names for tracing and error metadata and the AIP-derived
// Mutating flag for policy, and nothing else about the RPC.
type Method struct {
	// Name is the RPC's own name, e.a. "GetArtist".
	Name string

	// FullName is the fully-qualified proto method name, which is what a
	// tracing span and an ErrorInfo's metadata report.
	FullName string

	// Service is the fully-qualified proto service name.
	Service string

	// Pattern is the method's AIP classification, which is what a policy
	// selector dispatches on.
	Pattern Pattern

	// Mutating reports whether the method changes state, derived by the
	// generator from the method's AIP pattern rather than from its name — so a
	// policy written against it covers a method added later.
	Mutating bool

	// ServerStream reports whether the method streams its response, which
	// decides whether the status line may be written when the response opens
	// and whether a streaming-only codec such as SSE is a legal choice.
	//
	// There is no client-stream counterpart: HTTP transcoding has no honest
	// mapping for one, so the generator rejects the binding rather than
	// emitting a handler that cannot work.
	ServerStream bool
}
