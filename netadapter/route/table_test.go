package route_test

// table_test.go covers resolution: the three outcomes, and the custom-verb
// retry that a general-purpose router gets wrong.

import (
	"testing"

	"github.com/the-protobuf-project/http/netadapter/route"
)

// testTable is a small table in the order the generator would emit it: most
// specific first, verb-bearing routes ahead of their verbless twins.
func testTable() *route.Table {
	trackSegments := []route.Match{
		route.Literal("v1"), route.Literal("artists"), route.Single(),
		route.Literal("tracks"), route.Single(),
	}
	trackCapture := []route.Capture{{Field: []string{"name"}, JSON: "name", Start: 1, End: 5}}

	return route.NewTable([]route.Route{
		{
			Method: "POST", Segments: trackSegments, Verb: "withdraw",
			Captures: trackCapture, Template: "/v1/{name=artists/*/tracks/*}:withdraw",
			Handler: 0,
		},
		{
			Method: "GET", Segments: trackSegments, Captures: trackCapture,
			Template: "/v1/{name=artists/*/tracks/*}", Handler: 1,
		},
		{
			Method: "DELETE", Segments: trackSegments, Captures: trackCapture,
			Template: "/v1/{name=artists/*/tracks/*}", Handler: 2,
		},
	}, []route.Method{
		{Name: "WithdrawTrack", FullName: "music.v1.TrackService.WithdrawTrack", Pattern: route.PatternCustom, Mutating: true},
		{Name: "GetTrack", FullName: "music.v1.TrackService.GetTrack", Pattern: route.PatternGet},
		{Name: "DeleteTrack", FullName: "music.v1.TrackService.DeleteTrack", Pattern: route.PatternDelete, Mutating: true},
	})
}

func TestMatchedResolutionDecodesCaptures(t *testing.T) {
	resolved := testTable().Resolve("GET", "/v1/artists/miles/tracks/so-what")
	if resolved.Outcome != route.Matched {
		t.Fatalf("outcome = %v, want Matched", resolved.Outcome)
	}

	captures, err := resolved.Captures()
	if err != nil {
		t.Fatalf("captures: %v", err)
	}
	if captures["name"] != "artists/miles/tracks/so-what" {
		t.Errorf("name = %q, want the full resource name", captures["name"])
	}
}

func TestVerbIsOnlyPeeledForARouteThatAskedForOne(t *testing.T) {
	table := testTable()

	// A registered verb resolves to the verb-bearing route.
	withdraw := table.Resolve("POST", "/v1/artists/miles/tracks/so-what:withdraw")
	if withdraw.Outcome != route.Matched || withdraw.Route.Handler != 0 {
		t.Fatalf("withdraw resolved to %v/%d, want the verb route", withdraw.Outcome, handlerOf(withdraw))
	}
	if withdraw.Verb != "withdraw" {
		t.Errorf("verb = %q, want withdraw", withdraw.Verb)
	}

	// An unregistered one is not stripped. ":" is legal in a resource id, so the
	// final segment is retried whole and binds including the suffix — which is
	// the rule: an adapter must not peel a verb no route asked for.
	//
	// This is exactly what a general-purpose router gets wrong in the other
	// direction: it accepts `/v1/{name}:withdraw` as an ordinary route and folds
	// ":withdraw" into the name even when a route did ask for it.
	unknown := table.Resolve("GET", "/v1/artists/miles/tracks/so-what:unknown")
	if unknown.Outcome != route.Matched {
		t.Fatalf("outcome = %v, want Matched: the colon is part of the id", unknown.Outcome)
	}
	if unknown.Verb != "" {
		t.Errorf("verb = %q, want none peeled", unknown.Verb)
	}

	captures, err := unknown.Captures()
	if err != nil {
		t.Fatalf("captures: %v", err)
	}
	if captures["name"] != "artists/miles/tracks/so-what:unknown" {
		t.Errorf("name = %q, want the suffix kept as data", captures["name"])
	}
}

func TestMethodNotAllowedNamesEveryBoundMethod(t *testing.T) {
	// The Allow header is what a client needs to recover, and collapsing this
	// into a generic failure is how grpc-gateway turns a 405 into a 501.
	resolved := testTable().Resolve("PUT", "/v1/artists/miles/tracks/so-what")
	if resolved.Outcome != route.MethodNotAllowed {
		t.Fatalf("outcome = %v, want MethodNotAllowed", resolved.Outcome)
	}

	want := map[string]bool{"GET": true, "DELETE": true}
	if len(resolved.Allow) != len(want) {
		t.Fatalf("Allow = %v, want exactly %v", resolved.Allow, want)
	}
	for _, method := range resolved.Allow {
		if !want[method] {
			t.Errorf("Allow names %q, which is not bound to this path", method)
		}
	}
}

func TestMethodLookupRejectsAnIndexTheTableDoesNotHave(t *testing.T) {
	// A missing index means the route table and the method table disagree, which
	// is a generator bug. Reporting it beats panicking on the request path.
	if _, ok := testTable().Method(99); ok {
		t.Error("an out-of-range handler index was accepted")
	}
}

// handlerOf reports a resolution's handler index, or -1.
func handlerOf(resolution route.Resolution) int {
	if resolution.Route == nil {
		return -1
	}
	return resolution.Route.Handler
}
