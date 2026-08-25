package music_test

// streaming_test.go covers README §6.2, the rule the rest of the protocol is
// arranged to make satisfiable: an adapter must not report a 2xx for an RPC that
// did not succeed.
//
// Both halves are asserted here. A stream that fails before its first message
// keeps a real status, because the status line was never committed. One that
// fails after must truncate, because it cannot be unspent.

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/the-protobuf-project/grpc-gateway-rs/examples/music-go"
)

// serveStream runs a streaming request, reporting whether the handler
// terminated the response abnormally.
//
// net/http signals that termination by panicking with http.ErrAbortHandler,
// which a real server turns into RST_STREAM or a close without the final chunk.
// A recorder has no connection to abort, so catching the panic here is how a
// test observes the same decision.
func serveStream(t *testing.T, target string) (*http.Response, bool) {
	t.Helper()

	recorder := httptest.NewRecorder()
	truncated := false

	func() {
		defer func() {
			if recovered := recover(); recovered != nil {
				if recovered != http.ErrAbortHandler {
					panic(recovered)
				}
				truncated = true
			}
		}()
		request := httptest.NewRequest(http.MethodGet, target, nil)
		music.NewAdapter(music.SeededCatalog()).ServeHTTP(recorder, request)
	}()

	return recorder.Result(), truncated
}

func TestStreamSucceedsAsAJSONArray(t *testing.T) {
	response, truncated := serveStream(t, "/v1/artists/miles/tracks:watch")
	if truncated {
		t.Fatal("a successful stream was truncated")
	}
	if response.StatusCode != http.StatusOK {
		t.Fatalf("status = %d, want 200", response.StatusCode)
	}

	body := readBody(t, response)
	// Valid JSON once complete, and parseable by a streaming reader throughout.
	// This is how Google's own REST endpoints stream.
	var messages []map[string]any
	if err := json.Unmarshal([]byte(body), &messages); err != nil {
		t.Fatalf("body is not a JSON array: %v\n%s", err, body)
	}
	if len(messages) != 2 {
		t.Errorf("got %d messages, want the artist's 2 tracks", len(messages))
	}
}

func TestStreamFailingBeforeFirstMessageKeepsItsRealStatus(t *testing.T) {
	// The status line is not written when the stream opens, so a failure here —
	// authorization, validation, quota, not-found, which is the overwhelming
	// majority of real failures — produces an ordinary error response.
	response, truncated := serveStream(t, "/v1/artists/miles/tracks:watch?failAfter=0")
	if truncated {
		t.Fatal("a stream that had written nothing was truncated; its status was still available")
	}
	if response.StatusCode != http.StatusServiceUnavailable {
		t.Fatalf("status = %d, want 503", response.StatusCode)
	}

	body := decodeEnvelope(t, response)
	if body.Error.Status != "UNAVAILABLE" {
		t.Errorf("status = %q, want UNAVAILABLE", body.Error.Status)
	}
	if body.Error.Code != http.StatusServiceUnavailable {
		t.Errorf("envelope code = %d, want 503", body.Error.Code)
	}
}

func TestStreamFailingAfterCommitTruncates(t *testing.T) {
	// The status line is spent and no protocol can unspend it. All four things
	// must happen: an in-band error frame, the trailers, the abnormal
	// termination, and a server-side record.
	response, truncated := serveStream(t, "/v1/artists/miles/tracks:watch?failAfter=1")

	if !truncated {
		t.Fatal("a stream that failed after committing closed cleanly, reporting success for a failed RPC")
	}
	if response.StatusCode != http.StatusOK {
		t.Fatalf("status = %d, want the 200 that was already committed", response.StatusCode)
	}

	body := readBody(t, response)
	if !strings.Contains(body, `"error"`) {
		t.Errorf("body carries no terminal error frame:\n%s", body)
	}
	if !strings.Contains(body, "UNAVAILABLE") {
		t.Errorf("error frame does not name the status:\n%s", body)
	}

	// Advertised in the headers, because an intermediary that has not been told
	// to expect trailers is entitled to drop them.
	if advertised := response.Header.Get("Trailer"); !strings.Contains(advertised, "grpc-status") {
		t.Errorf("Trailer = %q, want grpc-status advertised", advertised)
	}
	if status := response.Trailer.Get("grpc-status"); status != "14" {
		t.Errorf("grpc-status trailer = %q, want 14 (UNAVAILABLE)", status)
	}
}

func TestStreamDefersItsHeadersUntilTheFirstMessage(t *testing.T) {
	// The deferral is what makes the previous test's "before" case possible at
	// all: a Content-Type written when the stream opened would have committed
	// the status with it.
	response, _ := serveStream(t, "/v1/artists/miles/tracks:watch?failAfter=0")
	if got := response.Header.Get("Content-Type"); got != "application/json" {
		t.Errorf("Content-Type = %q, want the error response's own", got)
	}
	if got := response.Header.Get("Trailer"); got != "" {
		t.Errorf("Trailer = %q, want none: no stream was ever opened", got)
	}
}

// readBody reads a response body as a string.
func readBody(t *testing.T, response *http.Response) string {
	t.Helper()

	var out strings.Builder
	buffer := make([]byte, 4096)
	for {
		n, err := response.Body.Read(buffer)
		out.Write(buffer[:n])
		if err != nil {
			break
		}
	}
	return out.String()
}
