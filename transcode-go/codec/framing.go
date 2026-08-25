package codec

// framing.go says how a sequence of messages is delimited on the wire.

// Framing is how a codec delimits a server-streaming response.
//
// Framing is a property of the codec rather than a separate negotiation axis:
// asking for application/json and asking for a JSON array of results are the
// same request, and splitting them would let a client select a combination that
// has no meaning, like SSE carrying length-prefixed protobuf.
type Framing uint8

const (
	// JSONArray writes one JSON array incrementally: "[" with the first
	// message, "," before each subsequent one, "]" at the end.
	//
	// The response is valid JSON once complete and parseable by a streaming
	// reader throughout. This is how Google's own REST endpoints stream, and it
	// is the default.
	JSONArray Framing = iota

	// SSE is Server-Sent Events: "event: message" and a "data:" line per
	// message.
	//
	// Streaming only. A unary request that selects it is rejected, because a
	// single-message event stream is a worse answer than a plain body.
	SSE

	// LineDelimited writes one compact JSON value per line.
	//
	// What grpc-gateway emits. Offered for clients already written against it.
	LineDelimited

	// LengthPrefixed writes a four-byte big-endian length followed by that many
	// bytes, per message.
	//
	// This is gRPC's message framing minus the compression flag, and the only
	// sensible choice for a binary codec: line-delimiting bytes that may
	// themselves contain a newline does not work.
	LengthPrefixed
)

// AllowsUnary reports whether a unary response may use this framing.
//
// SSE is the exception: it exists to carry a sequence of events, and a one-event
// stream is strictly worse for the client than a normal body.
func (f Framing) AllowsUnary() bool { return f != SSE }

// Open returns the bytes written before the first message of a stream.
func (f Framing) Open() []byte {
	if f == JSONArray {
		return []byte("[")
	}
	return nil
}

// Separator returns the bytes written between two consecutive messages.
//
// For [JSONArray] this is the array separator; for the line-oriented framings it
// terminates the preceding message. SSE puts its separator after each event
// instead, so it has none here.
func (f Framing) Separator() []byte {
	switch f {
	case JSONArray:
		return []byte(",")
	case LineDelimited:
		return []byte("\n")
	}
	return nil
}

// Close returns the bytes written after the final message of a successful
// stream.
//
// A stream that fails mid-flight does not get this: it is terminated abnormally
// instead, which is the only signal HTTP has left once the status line is spent.
// See README §6.2.
func (f Framing) Close() []byte {
	switch f {
	case JSONArray:
		return []byte("]")
	case LineDelimited:
		return []byte("\n")
	}
	return nil
}

// String names the framing, for diagnostics.
func (f Framing) String() string {
	switch f {
	case JSONArray:
		return "json-array"
	case SSE:
		return "sse"
	case LineDelimited:
		return "line-delimited"
	case LengthPrefixed:
		return "length-prefixed"
	}
	return "unknown"
}
