package stream

// termination.go says how a stream ended and what the transport must do about
// it.

import "github.com/the-protobuf-project/grpc-gateway-rs/netadapter/apierr"

// Outcome is how a stream ended.
type Outcome uint8

const (
	// Complete means the stream finished. Write the closing frame and the
	// trailers, then end the body normally.
	Complete Outcome = iota

	// Deferred means the stream failed before any message went out, so the
	// status line was never committed. Nothing streaming-specific has happened
	// yet: render it as an ordinary error response with its real status.
	Deferred

	// Truncate means the stream failed after committing. The status line is
	// spent.
	//
	// The transport must write the frame, send the trailers, and then terminate
	// the body abnormally — RST_STREAM with INTERNAL_ERROR on HTTP/2 and
	// HTTP/3, or closing without the terminating zero-length chunk on HTTP/1.1.
	//
	// Truncation is the only signal left: it makes curl exit non-zero and
	// fetch() reject, where a clean close would not.
	Truncate
)

// Termination is how a stream ended, with what the transport needs to act on it.
//
// This is the value that carries README §6.2 out to the listener. A transport
// that ignores [Truncate] and closes cleanly reports success for a failed RPC,
// which is the bug the whole design exists to close.
type Termination struct {
	// Outcome is which of the three cases this is.
	Outcome Outcome

	// Bytes are the closing frame for [Complete], or the in-band error frame
	// for [Truncate]. Empty for [Deferred], which has written nothing.
	Bytes []byte

	// Trailers describe the outcome. Meaningless for [Deferred], which is
	// rendered as an ordinary response.
	Trailers Trailers

	// Err is the failure, set for [Deferred] and [Truncate]. For [Truncate] it
	// is carried for the operator's log: the client's view of it is necessarily
	// degraded, so the server's must not be.
	Err *apierr.Error
}

// RequiresTruncation reports whether the body must be terminated abnormally.
func (t Termination) RequiresTruncation() bool { return t.Outcome == Truncate }

// HasTrailers reports whether there are trailers to emit.
func (t Termination) HasTrailers() bool { return t.Outcome != Deferred }
