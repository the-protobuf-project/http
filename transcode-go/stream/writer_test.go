package stream_test

// writer_test.go covers README §6.2 at the level of the state machine, which is
// where the rule is actually decided. The end-to-end half lives in the music
// example's streaming tests.

import (
	"strings"
	"testing"

	"github.com/the-protobuf-project/http/transcode-go/apierr"
	"github.com/the-protobuf-project/http/transcode-go/codec"
	"github.com/the-protobuf-project/http/transcode-go/stream"
)

// encodeError renders an envelope, standing in for the negotiated codec.
func encodeError(err *apierr.Error) []byte {
	body, marshalErr := err.JSON()
	if marshalErr != nil {
		return []byte(`{"error":{}}`)
	}
	return body
}

func TestFailureBeforeTheFirstMessageIsDeferred(t *testing.T) {
	// Nothing has been written, so the status line is still ours: this is
	// rendered as an ordinary error response with its real status.
	writer := stream.NewWriter(codec.JSONArray, "application/json")

	termination := writer.Fail(apierr.New(apierr.PermissionDenied, "No."), encodeError)
	if termination.Outcome != stream.Deferred {
		t.Fatalf("outcome = %v, want Deferred", termination.Outcome)
	}
	if termination.RequiresTruncation() {
		t.Error("a stream that wrote nothing asked to be truncated")
	}
	if termination.HasTrailers() {
		t.Error("a deferred failure emitted trailers; it is an ordinary response")
	}
}

func TestFailureAfterCommitTruncates(t *testing.T) {
	// The status line is spent and no protocol can unspend it. All three
	// artefacts must be present: the in-band frame, the trailers, and the
	// instruction to terminate abnormally.
	writer := stream.NewWriter(codec.JSONArray, "application/json")
	writer.Message([]byte(`{"track":{}}`))

	termination := writer.Fail(apierr.New(apierr.Unavailable, "Gone."), encodeError)
	if termination.Outcome != stream.Truncate {
		t.Fatalf("outcome = %v, want Truncate", termination.Outcome)
	}
	if !termination.RequiresTruncation() {
		t.Error("a committed failure closed cleanly, reporting success for a failed RPC")
	}
	if got := string(termination.Bytes); !strings.Contains(got, "UNAVAILABLE") {
		t.Errorf("error frame = %q, want the status named", got)
	}
	if termination.Trailers.Status != int32(apierr.Unavailable) {
		t.Errorf("grpc-status = %d, want %d", termination.Trailers.Status, apierr.Unavailable)
	}
}

func TestJSONArrayIsWellFormedAtEveryStop(t *testing.T) {
	// Valid JSON once complete and parseable by a streaming reader throughout,
	// which is how Google's own REST endpoints stream.
	writer := stream.NewWriter(codec.JSONArray, "application/json")

	var body strings.Builder
	body.Write(writer.Message([]byte(`{"a":1}`)))
	body.Write(writer.Message([]byte(`{"a":2}`)))
	body.Write(writer.Finish().Bytes)

	if got := body.String(); got != `[{"a":1},{"a":2}]` {
		t.Errorf("body = %s, want a well-formed array", got)
	}
}

func TestEmptyStreamStillClosesAsAnArray(t *testing.T) {
	writer := stream.NewWriter(codec.JSONArray, "application/json")

	if got := string(writer.Finish().Bytes); got != "[]" {
		t.Errorf("body = %q, want []", got)
	}
}

func TestSSEUsesADistinctEventNameForFailure(t *testing.T) {
	// So a browser handler can bind to it rather than having to inspect every
	// message to find out whether it was the error.
	writer := stream.NewWriter(codec.SSE, "text/event-stream")
	writer.Message([]byte(`{"a":1}`))

	frame := string(writer.Fail(apierr.New(apierr.Internal, "Boom."), encodeError).Bytes)
	if !strings.HasPrefix(frame, "event: error\n") {
		t.Errorf("frame = %q, want an error event", frame)
	}
}

func TestGrpcMessageIsPercentEncoded(t *testing.T) {
	// A raw newline in a header value is a request-smuggling vector rather than
	// merely a formatting problem, and a status message routinely carries a
	// resource name or a quoted value.
	writer := stream.NewWriter(codec.JSONArray, "application/json")
	writer.Message([]byte(`{}`))

	trailers := writer.Fail(apierr.New(apierr.Internal, "line one\nline two"), encodeError).Trailers
	if strings.Contains(trailers.Message, "\n") {
		t.Errorf("grpc-message = %q, want the newline encoded", trailers.Message)
	}
	if !strings.Contains(trailers.Message, "%0A") {
		t.Errorf("grpc-message = %q, want %%0A", trailers.Message)
	}
}

func TestWritingAfterTheEndIsDroppedNotFatal(t *testing.T) {
	// A streaming handler is usually a goroutine feeding a channel, and killing
	// the process because one raced past its own termination is a worse outcome
	// than dropping the message and letting the trailers stand.
	writer := stream.NewWriter(codec.JSONArray, "application/json")
	writer.Finish()

	if got := writer.Message([]byte(`{"a":1}`)); got != nil {
		t.Errorf("wrote %q after the stream ended", got)
	}
}
