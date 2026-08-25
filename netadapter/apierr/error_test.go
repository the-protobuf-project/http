package apierr_test

// error_test.go covers the AIP-193 envelope, which is where this project most
// visibly departs from grpc-gateway.

import (
	"encoding/json"
	"testing"
	"time"

	"github.com/the-protobuf-project/grpc-gateway-rs/netadapter/apierr"
)

// decode renders an error and parses it back, as a client would.
func decode(t *testing.T, err *apierr.Error) map[string]any {
	t.Helper()

	raw, marshalErr := err.JSON()
	if marshalErr != nil {
		t.Fatalf("render: %v", marshalErr)
	}

	var envelope map[string]any
	if err := json.Unmarshal(raw, &envelope); err != nil {
		t.Fatalf("parse: %v\n%s", err, raw)
	}

	body, ok := envelope["error"].(map[string]any)
	if !ok {
		t.Fatalf("envelope has no error object: %s", raw)
	}
	return body
}

func TestEnvelopeCodeIsTheHTTPStatusNotTheGRPCCode(t *testing.T) {
	// The single difference that started this project. grpc-gateway serializes
	// the raw google.rpc.Status, whose code field holds the canonical code's
	// number — 3 for a bad request, which is not an HTTP status at all.
	body := decode(t, apierr.New(apierr.InvalidArgument, "Bad field."))

	if got := body["code"].(float64); got != 400 {
		t.Errorf("code = %v, want 400", got)
	}
	if got := body["status"].(string); got != "INVALID_ARGUMENT" {
		t.Errorf("status = %q, want INVALID_ARGUMENT", got)
	}
}

func TestPromotedStatusIsWhatTheEnvelopeReports(t *testing.T) {
	// A routing failure keeps its 405 rather than following UNIMPLEMENTED to
	// 501, and the body must agree with the status line.
	err := apierr.MethodNotAllowed("PUT", []string{"GET", "DELETE"}, "test")
	body := decode(t, err)

	if err.HTTP != 405 {
		t.Errorf("HTTP = %d, want 405", err.HTTP)
	}
	if got := body["code"].(float64); got != 405 {
		t.Errorf("envelope code = %v, want 405", got)
	}
	if got := err.Headers().Get("Allow"); got != "GET, DELETE" {
		t.Errorf("Allow = %q, want the bound methods", got)
	}
}

func TestSynthesisedErrorInfoGoesFirst(t *testing.T) {
	// AIP-193 requires exactly one, and it is the detail a caller reads to
	// decide what to do — so it leads the array, which has no other ordering.
	err := apierr.New(apierr.NotFound, "Gone.").
		WithDetail(apierr.RequestInfo{RequestID: "abc"}).
		EnsureErrorInfo("test")

	details := decode(t, err)["details"].([]any)
	first := details[0].(map[string]any)

	if first["@type"] != "type.googleapis.com/google.rpc.ErrorInfo" {
		t.Errorf("first detail = %v, want the ErrorInfo", first["@type"])
	}
	if first["reason"] != "NOT_FOUND" {
		t.Errorf("reason = %v, want the code name", first["reason"])
	}
}

func TestExistingErrorInfoIsNotDuplicated(t *testing.T) {
	err := apierr.New(apierr.NotFound, "Gone.").
		WithErrorInfo("RESOURCE_MISSING", "test", nil).
		EnsureErrorInfo("test")

	var count int
	for _, detail := range decode(t, err)["details"].([]any) {
		if detail.(map[string]any)["@type"] == "type.googleapis.com/google.rpc.ErrorInfo" {
			count++
		}
	}
	if count != 1 {
		t.Errorf("ErrorInfo count = %d, want exactly 1", count)
	}
}

func TestRetryInfoProjectsToRetryAfterRoundingUp(t *testing.T) {
	// Rounding down would invite a retry the server is still not ready for.
	err := apierr.New(apierr.ResourceExhausted, "Slow down.").
		WithDetail(apierr.RetryInfo{RetryDelay: 1500 * time.Millisecond})

	if got := err.Headers().Get("Retry-After"); got != "2" {
		t.Errorf("Retry-After = %q, want 2", got)
	}
}

func TestChallengeEscapesTheQuotedString(t *testing.T) {
	// grpc-gateway sets this header to the raw status message, which violates
	// the RFC 7235 grammar as soon as a message contains a quote — and a message
	// describing a rejected token very often does.
	err := apierr.New(apierr.Unauthenticated, `Token "abc" expired`).
		WithErrorInfo("CREDENTIAL_INVALID", "api.example.com", nil)

	challenge := err.Headers().Get("WWW-Authenticate")
	want := `Bearer realm="api.example.com", error="invalid_token", error_description="Token \"abc\" expired"`
	if challenge != want {
		t.Errorf("challenge =\n%s\nwant\n%s", challenge, want)
	}
}

func TestDebugInfoIsStrippedOnRequest(t *testing.T) {
	// A DebugInfo describes the shape of the service, so it leaves only when a
	// deployment has explicitly asked for it.
	err := apierr.New(apierr.Internal, "Boom.").
		WithDetail(apierr.DebugInfo{Detail: "table users is missing"}).
		EnsureErrorInfo("test").
		StripDebugInfo()

	for _, detail := range decode(t, err)["details"].([]any) {
		if detail.(map[string]any)["@type"] == "type.googleapis.com/google.rpc.DebugInfo" {
			t.Error("DebugInfo survived stripping")
		}
	}
}

func TestLocalizedMessageSupersedesTheEnvelopeMessage(t *testing.T) {
	err := apierr.New(apierr.NotFound, "Not found.").
		WithDetail(apierr.LocalizedMessage{Locale: "fr-FR", Message: "Introuvable."})

	if got := decode(t, err)["message"].(string); got != "Introuvable." {
		t.Errorf("message = %q, want the localized one", got)
	}
}

func TestUnknownCodeDegradesToUnknown(t *testing.T) {
	// A code from a newer peer must not become a nonsense status.
	if got := apierr.FromNumber(99); got != apierr.Unknown {
		t.Errorf("code = %v, want Unknown", got)
	}
	if got := apierr.FromNumber(99).HTTPStatus(); got != 500 {
		t.Errorf("status = %d, want 500", got)
	}
}
