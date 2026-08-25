package music_test

// conformance_test.go asserts the protocol, not the catalog.
//
// Every case here is a rule from the README that the Rust runtime is held to as
// well. They are written against the handler rather than a live socket so they
// run without a port, and against the same seeded catalog the Rust example
// serves, so a divergence between the two runtimes shows up as a differing
// assertion rather than as a subtle behavioural drift nobody looks for.

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/the-protobuf-project/http/examples/music-go"
)

// serve runs one request against a freshly seeded handler.
func serve(t *testing.T, method, target string, body string, header http.Header) *http.Response {
	t.Helper()

	request := httptest.NewRequest(method, target, strings.NewReader(body))
	for name, values := range header {
		for _, value := range values {
			request.Header.Add(name, value)
		}
	}

	recorder := httptest.NewRecorder()
	music.NewHandler(music.SeededCatalog()).ServeHTTP(recorder, request)
	return recorder.Result()
}

// envelope is the AIP-193 error body, for asserting on its shape.
type envelope struct {
	Error struct {
		Code    int              `json:"code"`
		Message string           `json:"message"`
		Status  string           `json:"status"`
		Details []map[string]any `json:"details"`
	} `json:"error"`
}

// decodeEnvelope reads an error body, failing the test if it is not one.
func decodeEnvelope(t *testing.T, response *http.Response) envelope {
	t.Helper()

	var got envelope
	if err := json.NewDecoder(response.Body).Decode(&got); err != nil {
		t.Fatalf("response body is not an AIP-193 envelope: %v", err)
	}
	return got
}

func TestUnknownPathIsNotFound(t *testing.T) {
	response := serve(t, http.MethodGet, "/v1/nothing", "", nil)
	if response.StatusCode != http.StatusNotFound {
		t.Fatalf("status = %d, want 404", response.StatusCode)
	}

	// The envelope's code is the HTTP status, not the gRPC code. That single
	// difference is the bug this project started from: grpc-gateway reports 5
	// here, which is not an HTTP status at all.
	body := decodeEnvelope(t, response)
	if body.Error.Code != http.StatusNotFound {
		t.Errorf("envelope code = %d, want 404", body.Error.Code)
	}
	if body.Error.Status != "NOT_FOUND" {
		t.Errorf("envelope status = %q, want NOT_FOUND", body.Error.Status)
	}
}

func TestWrongMethodIsFourOhFiveWithAllow(t *testing.T) {
	// grpc-gateway routes this through UNIMPLEMENTED and its status table maps
	// it back out as 501, losing both the status and the header a client needs
	// to recover.
	response := serve(t, http.MethodPut, "/v1/artists/miles", "", nil)
	if response.StatusCode != http.StatusMethodNotAllowed {
		t.Fatalf("status = %d, want 405", response.StatusCode)
	}

	allow := response.Header.Get("Allow")
	for _, want := range []string{"GET", "PATCH", "DELETE"} {
		if !strings.Contains(allow, want) {
			t.Errorf("Allow = %q, missing %s", allow, want)
		}
	}
}

func TestEveryErrorCarriesExactlyOneErrorInfo(t *testing.T) {
	// AIP-193 requires it, and a caller who cannot tell which service failed,
	// or why, cannot act on the error.
	body := decodeEnvelope(t, serve(t, http.MethodGet, "/v1/artists/nobody", "", nil))

	var found int
	for _, detail := range body.Error.Details {
		if detail["@type"] == "type.googleapis.com/google.rpc.ErrorInfo" {
			found++
			if detail["domain"] != music.Domain() {
				t.Errorf("ErrorInfo domain = %v, want %q", detail["domain"], music.Domain())
			}
		}
	}
	if found != 1 {
		t.Errorf("ErrorInfo count = %d, want exactly 1", found)
	}
}
