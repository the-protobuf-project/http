package route_test

// route_test.go covers the matcher: the four template shapes, the precedence
// rules, and the percent-decoding exception the whole design turns on.

import (
	"errors"
	"testing"

	"github.com/the-protobuf-project/grpc-gateway-rs/netadapter/route"
)

// artistsAny is `/v1/artists/*`, binding the whole thing as `name`.
var artistsAny = route.Route{
	Method:   "GET",
	Segments: []route.Match{route.Literal("v1"), route.Literal("artists"), route.Single()},
	Captures: []route.Capture{{Field: []string{"name"}, JSON: "name", Start: 1, End: 3}},
	Template: "/v1/{name=artists/*}",
}

// anyRest is `/v1/**`, binding everything after the version.
var anyRest = route.Route{
	Method:   "GET",
	Segments: []route.Match{route.Literal("v1"), route.Multi()},
	Captures: []route.Capture{{Field: []string{"name"}, JSON: "name", Start: 1, End: route.ToEnd}},
	Template: "/v1/{name=**}",
}

func TestSingleMatchesExactlyOneSegment(t *testing.T) {
	for _, tc := range []struct {
		path  string
		match bool
	}{
		{"/v1/artists/miles", true},
		// A "*" binds exactly one component, and an empty one — from a doubled
		// or trailing slash — is not one.
		{"/v1/artists/", false},
		{"/v1/artists", false},
		{"/v1/artists/miles/tracks", false},
	} {
		segments, verb := route.SplitPath(tc.path, false)
		if got := artistsAny.Matches(segments, verb); got != tc.match {
			t.Errorf("%s: matched = %v, want %v", tc.path, got, tc.match)
		}
	}
}

func TestMultiMatchesZeroOrMoreSegments(t *testing.T) {
	for _, tc := range []struct {
		path  string
		match bool
		value string
	}{
		{"/v1/artists/miles/tracks/so-what", true, "artists/miles/tracks/so-what"},
		{"/v1/artists", true, "artists"},
		// Zero segments is a match, and yields an empty value rather than an
		// error.
		{"/v1", true, ""},
	} {
		segments, verb := route.SplitPath(tc.path, false)
		if !anyRest.Matches(segments, verb) {
			t.Errorf("%s: did not match", tc.path)
			continue
		}

		value, err := anyRest.Capture(anyRest.Captures[0], segments)
		if err != nil {
			t.Errorf("%s: capture: %v", tc.path, err)
			continue
		}
		if value != tc.value {
			t.Errorf("%s: captured %q, want %q", tc.path, value, tc.value)
		}
	}
}

func TestCaptureLeavesEncodedSlashAlone(t *testing.T) {
	// The rule the design turns on. "/" separates the segments of an AIP-122
	// resource name, so decoding %2F would make "artists/a%2Fb" and
	// "artists/a/b" arrive identical, and nothing downstream could tell a
	// two-segment name holding a slash from a three-segment one.
	segments, verb := route.SplitPath("/v1/artists/a%2Fb", false)
	if !artistsAny.Matches(segments, verb) {
		t.Fatal("did not match")
	}

	value, err := artistsAny.Capture(artistsAny.Captures[0], segments)
	if err != nil {
		t.Fatalf("capture: %v", err)
	}
	if value != "artists/a%2Fb" {
		t.Errorf("captured %q, want the %%2F preserved", value)
	}
}

func TestCaptureDecodesEverythingElse(t *testing.T) {
	segments, _ := route.SplitPath("/v1/artists/miles%20davis", false)
	value, err := artistsAny.Capture(artistsAny.Captures[0], segments)
	if err != nil {
		t.Fatalf("capture: %v", err)
	}
	if value != "artists/miles davis" {
		t.Errorf("captured %q, want the space decoded", value)
	}
}

func TestUndecodableCaptureNamesItsField(t *testing.T) {
	// The field travels with the error so the caller can raise a FieldViolation
	// naming what the client actually sent, rather than a bare "malformed path".
	segments, _ := route.SplitPath("/v1/artists/mile%FF", false)

	_, err := artistsAny.Capture(artistsAny.Captures[0], segments)
	if err == nil {
		t.Fatal("invalid UTF-8 was accepted")
	}

	var captureErr *route.CaptureError
	if !errors.As(err, &captureErr) {
		t.Fatalf("error = %T, want *route.CaptureError", err)
	}
	if captureErr.Field != "name" {
		t.Errorf("field = %q, want name", captureErr.Field)
	}
	if captureErr.Err != route.ErrNotUTF8 {
		t.Errorf("kind = %v, want ErrNotUTF8", captureErr.Err)
	}
}
