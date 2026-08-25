package music_test

// routing_test.go covers the routing rules a general-purpose HTTP router cannot
// express, which is why this project carries its own matcher.

import (
	"encoding/json"
	"net/http"
	"strings"
	"testing"

	"github.com/the-protobuf-project/grpc-gateway-rs/examples/music-go"
)

func TestMultiSegmentCaptureBinds(t *testing.T) {
	// matchit, axum's router, rejects `/v1/{name=artists/*/tracks/*}` outright.
	// The compiled table walks it positionally instead.
	response := serve(t, http.MethodGet, "/v1/artists/miles/tracks/so-what", "", nil)
	if response.StatusCode != http.StatusOK {
		t.Fatalf("status = %d, want 200", response.StatusCode)
	}

	var track map[string]any
	if err := json.NewDecoder(response.Body).Decode(&track); err != nil {
		t.Fatalf("decode: %v", err)
	}
	if track["name"] != "artists/miles/tracks/so-what" {
		t.Errorf("name = %v, want the full resource name", track["name"])
	}
}

func TestCustomVerbIsNotPartOfTheResourceName(t *testing.T) {
	// The failure mode this guards against is the one a general-purpose router
	// has: it accepts `/v1/{name}:withdraw` as an ordinary route and silently
	// folds ":withdraw" into the captured name.
	response := serve(t, http.MethodPost,
		"/v1/artists/miles/tracks/so-what:withdraw", "{}",
		http.Header{"Content-Type": []string{"application/json"}})
	if response.StatusCode != http.StatusOK {
		t.Fatalf("status = %d, want 200", response.StatusCode)
	}

	var track map[string]any
	if err := json.NewDecoder(response.Body).Decode(&track); err != nil {
		t.Fatalf("decode: %v", err)
	}
	if track["name"] != "artists/miles/tracks/so-what" {
		t.Errorf("name = %v, want the verb stripped", track["name"])
	}
	if track["availability"] != "AVAILABILITY_UNAVAILABLE" {
		t.Errorf("availability = %v, want the track withdrawn", track["availability"])
	}
}

func TestUnregisteredVerbIsNotStripped(t *testing.T) {
	// A ":" is legal in a resource id, so a suffix no registered route asked for
	// must stay part of the name — and then simply not match anything.
	response := serve(t, http.MethodGet, "/v1/artists/miles:unknown", "", nil)
	if response.StatusCode != http.StatusNotFound {
		t.Fatalf("status = %d, want 404", response.StatusCode)
	}
}

func TestPercentEncodedSlashSurvivesCapture(t *testing.T) {
	// README §1.2 step 4: every escape decodes except %2F, because "/"
	// separates the segments of an AIP-122 resource name. Decoding it would make
	// "artists/a%2Fb" and "artists/a/b" arrive identical.
	response := serve(t, http.MethodGet, "/v1/artists/a%2Fb", "", nil)
	if response.StatusCode != http.StatusNotFound {
		t.Fatalf("status = %d, want 404 (no such artist)", response.StatusCode)
	}

	body := decodeEnvelope(t, response)
	if !strings.Contains(body.Error.Message, "a%2Fb") {
		t.Errorf("message = %q, want the %%2F left encoded", body.Error.Message)
	}
}

func TestUndecodableCaptureIsFourHundredNotFourOhFour(t *testing.T) {
	// %FF is a well-formed escape that decodes to a byte no UTF-8 string can
	// hold. The path matched a route; the value is what is wrong. Reporting
	// "not found" would send a caller looking for a resource when the fix is to
	// fix their encoding.
	//
	// The sibling case — a syntactically malformed escape like %zz — never
	// reaches the adapter on this runtime: net/http rejects the request line
	// itself and answers with its own plain-text 400. The status is right and
	// the body is not an AIP-193 envelope, which is a divergence from the Rust
	// runtime that no handler can close. See the runtime README.
	response := serve(t, http.MethodGet, "/v1/artists/mile%FF", "", nil)
	if response.StatusCode != http.StatusBadRequest {
		t.Fatalf("status = %d, want 400", response.StatusCode)
	}

	body := decodeEnvelope(t, response)
	if body.Error.Status != "INVALID_ARGUMENT" {
		t.Errorf("status = %q, want INVALID_ARGUMENT", body.Error.Status)
	}

	var reason string
	for _, detail := range body.Error.Details {
		if value, ok := detail["reason"].(string); ok {
			reason = value
		}
	}
	if reason != "MALFORMED_PATH" {
		t.Errorf("reason = %q, want MALFORMED_PATH", reason)
	}
}

func TestRoutesAreSortedMostSpecificFirst(t *testing.T) {
	// /v1/artists and /v1/{name=artists/*} both start with the same literal.
	// The generator sorts the table so the literal-only route cannot be shadowed
	// by the wildcard one.
	response := serve(t, http.MethodGet, "/v1/artists", "", nil)
	if response.StatusCode != http.StatusOK {
		t.Fatalf("status = %d, want 200", response.StatusCode)
	}

	var listed map[string]any
	if err := json.NewDecoder(response.Body).Decode(&listed); err != nil {
		t.Fatalf("decode: %v", err)
	}
	if _, ok := listed["artists"]; !ok {
		t.Errorf("body = %v, want a List response rather than a single artist", listed)
	}
}

func TestUnknownQueryParameterIsRejected(t *testing.T) {
	// The opposite of grpc-gateway, which discards them — turning a typo in an
	// update call into a silent no-op.
	response := serve(t, http.MethodGet, "/v1/artists?pagesize=2", "", nil)
	if response.StatusCode != http.StatusBadRequest {
		t.Fatalf("status = %d, want 400", response.StatusCode)
	}

	body := decodeEnvelope(t, response)
	var named bool
	for _, detail := range body.Error.Details {
		violations, ok := detail["fieldViolations"].([]any)
		if !ok {
			continue
		}
		for _, violation := range violations {
			if entry, ok := violation.(map[string]any); ok && entry["field"] == "pagesize" {
				named = true
			}
		}
	}
	if !named {
		t.Errorf("details = %v, want a FieldViolation naming pagesize", body.Error.Details)
	}
}

func TestDomainIsStampedOnGatewayErrors(t *testing.T) {
	if music.Domain() != "music.example.com" {
		t.Fatalf("domain = %q, want the one the protos were generated with", music.Domain())
	}
}
