package netadapter

// stream.go serves a server-streaming call, and is where the no-false-2xx rule
// meets net/http.

import (
	"net/http"

	"github.com/the-protobuf-project/http/netadapter/apierr"
	"github.com/the-protobuf-project/http/netadapter/stream"
)

// Stream is the handle a streaming handler writes messages through.
//
// It exists so a handler cannot commit the status line by accident: there is no
// way to reach the ResponseWriter through it, and the headers are written by
// the first [Stream.Send] rather than when the stream opens.
type Stream struct {
	// writer is the state machine enforcing README §6.2.
	writer *stream.Writer

	// response is the transport being written to.
	response http.ResponseWriter

	// encode renders the AIP-193 envelope with the negotiated codec, used only
	// when a stream fails after committing.
	encode func(*apierr.Error) []byte
}

// Send writes one already-encoded message, committing the status line if this
// is the first.
//
// The headers and the status go out here, not when the stream opened. A failure
// before the first Send therefore still has a real status, which covers
// authorization, validation, quota and not-found — the overwhelming majority of
// real failures.
func (s *Stream) Send(encoded []byte) error {
	first := !s.writer.Committed()
	frame := s.writer.Message(encoded)

	if first {
		s.writer.WriteHeaders(s.response.Header())
		s.response.WriteHeader(s.writer.Status())
	}
	if _, err := s.response.Write(frame); err != nil {
		return err
	}
	s.flush()
	return nil
}

// Keepalive writes a keepalive frame, for framings that have one.
//
// A handler calls it on an idle timer: an SSE connection that is merely waiting
// looks identical to one that has died, and intermediaries reap it.
func (s *Stream) Keepalive() {
	frame := s.writer.Keepalive()
	if len(frame) == 0 || !s.writer.Committed() {
		return
	}
	//nolint:errcheck // A failed keepalive means the peer is gone, which the
	// next Send reports properly.
	_, _ = s.response.Write(frame)
	s.flush()
}

// flush pushes buffered bytes to the client.
//
// A stream the client cannot see until it ends is not a stream, and Go's
// server buffers by default. A ResponseWriter that cannot flush is left alone
// rather than treated as an error: it is what a test recorder is.
func (s *Stream) flush() {
	if flusher, ok := s.response.(http.Flusher); ok {
		flusher.Flush()
	}
}

// serveStream dispatches a streaming call and terminates it per README §6.2.
//
// It returns the failure that ended the stream, for the completion phase. There
// is no response phase for a stream: by the time a hook could run, the headers
// have gone out with the first message, so there is nothing left to change.
func (a *Adapter) serveStream(w http.ResponseWriter, call *Call) *apierr.Error {
	dispatcher, ok := a.dispatch.(StreamDispatcher)
	if !ok {
		failure := a.asGatewayError(call.Errorf(apierr.Unimplemented,
			"This method streams its response, which this adapter was not built to serve."))
		errorReply(failure).Write(w)
		return failure
	}

	writer := stream.NewWriter(call.ResponseCodec.Framing, call.ResponseCodec.ContentType())
	out := &Stream{writer: writer, response: w, encode: a.encodeError}

	err := dispatcher.DispatchStream(call, out)
	if err == nil {
		return a.terminate(w, writer.Finish())
	}
	return a.terminate(w, writer.Fail(a.asGatewayError(err), a.encodeError))
}

// terminate acts on how a stream ended, returning the failure if there was one.
func (a *Adapter) terminate(w http.ResponseWriter, termination stream.Termination) *apierr.Error {
	if termination.Outcome == stream.Deferred {
		// Nothing was written, so the status line is still ours and the failure
		// is rendered as an ordinary error response with its real status.
		errorReply(termination.Err).Write(w)
		return termination.Err
	}

	//nolint:errcheck // The peer being gone changes nothing about what must be
	// written next; the trailers and the truncation still stand.
	_, _ = w.Write(termination.Bytes)
	termination.Trailers.Apply(w.Header())

	if !termination.RequiresTruncation() {
		return nil
	}

	a.options.Logger.Error(
		"stream failed after committing its status; truncating the body",
		"status", termination.Err.HTTP,
		"code", termination.Err.Code.String(),
		"message", termination.Err.Message,
	)

	// Truncation is the only signal HTTP has left once the status line is
	// spent, and it is what makes the failure observable: curl exits non-zero,
	// fetch() rejects, a Go client sees io.ErrUnexpectedEOF. ErrAbortHandler is
	// how a handler asks net/http for it — RST_STREAM on HTTP/2, and a close
	// without the terminating zero-length chunk on HTTP/1.1.
	//
	// The completion phase has already been deferred by the caller, so logging
	// and metrics still see this call: the panic unwinds through that defer
	// before net/http catches it.
	panic(http.ErrAbortHandler)
}

// encodeError renders an error envelope for an in-band stream frame.
//
// JSON regardless of the negotiated codec: the envelope is the one message in
// the protocol with no generated type behind it, and a client that cannot parse
// its own error frame learns nothing from it.
func (a *Adapter) encodeError(err *apierr.Error) []byte {
	body, marshalErr := err.JSON()
	if marshalErr != nil {
		return []byte(`{"error":{"code":500,"message":"Internal error.","status":"INTERNAL"}}`)
	}
	return body
}
