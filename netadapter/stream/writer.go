package stream

// writer.go is the stream state machine.

import (
	"net/http"

	"github.com/the-protobuf-project/grpc-gateway-rs/netadapter/apierr"
	"github.com/the-protobuf-project/grpc-gateway-rs/netadapter/codec"
)

// State is where a stream is in its life.
//
// The distinction that matters is [Pending] versus [Committed]: it is the
// difference between a failure that can still have a real status and one that
// can only be truncated.
type State uint8

const (
	// Pending means no message has gone out. The status line is uncommitted, so
	// a failure here is reported normally.
	Pending State = iota

	// Committed means at least one message has gone out. The status line is
	// spent.
	Committed

	// Done means the stream has ended.
	Done
)

// Writer writes a server-streaming response, enforcing README §6.2.
//
// The writer produces bytes but never sends them: a transport does that. This
// keeps the rule in one place and testable — the same state machine backs
// HTTP/1.1, HTTP/2 and HTTP/3, so none of them can implement it
// almost-correctly on its own.
type Writer struct {
	// framer renders bytes for the negotiated framing.
	framer *Framer

	// state is where the stream is in its life.
	state State

	// contentType is the negotiated codec's media type.
	contentType string
}

// NewWriter returns a writer for one negotiated codec.
func NewWriter(framing codec.Framing, contentType string) *Writer {
	return &Writer{framer: NewFramer(framing), state: Pending, contentType: contentType}
}

// State returns where the stream is in its life.
func (w *Writer) State() State { return w.state }

// Committed reports whether the status line has been spent.
func (w *Writer) Committed() bool { return w.state == Committed || w.state == Done }

// WriteHeaders sets the response headers, and must be called when the first
// message is ready rather than when the stream opens.
//
// Sending headers early is exactly how the status gets committed prematurely,
// which is the failure this whole package exists to prevent.
func (w *Writer) WriteHeaders(header http.Header) {
	header.Set("Content-Type", w.contentType)
	// Advertised in the headers, because an intermediary that has not been told
	// to expect trailers is entitled to drop them — and Go's server will not
	// send them without this either.
	header.Set("Trailer", Advertised)
	// A stream must not be cached: what a replay would serve is a prefix.
	header.Set("Cache-Control", "no-cache")
}

// Status is the status line, which for a stream that produced anything is
// always 200.
//
// The honesty comes from [Truncate], not from this: once a message is out, 200
// is already true of what was sent.
func (w *Writer) Status() int { return http.StatusOK }

// Message returns the bytes for one encoded message, committing the status line
// if this is the first.
//
// Writing to a stream that already ended returns nothing rather than panicking:
// a streaming handler is usually a goroutine feeding a channel, and killing the
// process because one raced past its own termination is a worse outcome than
// dropping the message and letting the trailers stand.
func (w *Writer) Message(encoded []byte) []byte {
	if w.state == Done {
		return nil
	}
	w.state = Committed
	return w.framer.Message(encoded)
}

// Keepalive returns a keepalive frame, for framings that have one.
func (w *Writer) Keepalive() []byte { return w.framer.Keepalive() }

// Finish ends a stream that completed.
func (w *Writer) Finish() Termination {
	w.state = Done
	return Termination{
		Outcome:  Complete,
		Bytes:    w.framer.Close(),
		Trailers: OKTrailers(),
	}
}

// Fail ends a stream that failed.
//
// encodeError renders the AIP-193 envelope with the negotiated codec. It is a
// function rather than pre-encoded bytes because it is only called in the
// committed case — a failure before the first message is rendered as an
// ordinary error response instead, by whatever handles unary errors.
//
// This is the whole of README §6.2 in one branch: before the commit, a real
// status; after it, an error frame plus trailers plus truncation.
func (w *Writer) Fail(err *apierr.Error, encodeError func(*apierr.Error) []byte) Termination {
	committed := w.Committed()
	w.state = Done

	if !committed {
		// Nothing has been written, so the status line is still ours.
		return Termination{Outcome: Deferred, Err: err}
	}

	return Termination{
		Outcome:  Truncate,
		Bytes:    w.framer.Error(encodeError(err)),
		Trailers: ErrorTrailers(err),
		Err:      err,
	}
}
