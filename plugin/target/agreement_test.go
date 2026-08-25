package target_test

// agreement_test.go is the check that the targets cannot describe different
// APIs.
//
// Two runtimes agreeing about what a request means is the property this whole
// design exists to protect, and it is not something a per-target golden file can
// establish: each would be internally consistent while describing a different
// route table. These tests extract what each target emitted and compare them to
// each other.

import (
	"regexp"
	"strings"
	"testing"
)

// templatePattern finds the path templates a generated table carries. Both
// targets emit them as a quoted string, which is what makes one extractor
// enough.
var templatePattern = regexp.MustCompile(`"(/v1/[^"]*)"`)

// extractTemplates returns the templates in the order the emitted table scans
// them.
//
// Order is the point, not just membership: a table that lists the same routes in
// a different order resolves the same request to a different method.
func extractTemplates(t *testing.T, files map[string]string, name string) []string {
	t.Helper()

	source, ok := files[name]
	if !ok {
		var have []string
		for file := range files {
			have = append(have, file)
		}
		t.Fatalf("no %s emitted; got %v", name, have)
	}

	var templates []string
	for _, match := range templatePattern.FindAllStringSubmatch(source, -1) {
		templates = append(templates, match[1])
	}
	if len(templates) == 0 {
		t.Fatalf("%s carries no path templates", name)
	}
	return templates
}

func TestTargetsEmitTheSameRoutesInTheSameOrder(t *testing.T) {
	// Scan order is resolution: the runtime takes the first match, so two
	// tables that disagree about order disagree about which method serves a
	// request that two routes could both match.
	rust := extractTemplates(t, generate(t, "rust"), "routes.rs")
	golang := extractTemplates(t, generate(t, "go"), "routes.go")

	if len(rust) != len(golang) {
		t.Fatalf("route counts differ: rust %d, go %d\nrust: %v\ngo:   %v",
			len(rust), len(golang), rust, golang)
	}
	for i := range rust {
		if rust[i] != golang[i] {
			t.Errorf("route %d differs: rust %q, go %q", i, rust[i], golang[i])
		}
	}
}

func TestTargetsAgreeOnHandlerIndices(t *testing.T) {
	// A handler index is a promise between the table and the dispatch switch. If
	// the two targets numbered methods differently, a route table generated for
	// one runtime would dispatch to the wrong method in the other — silently,
	// because both indices are valid.
	rustMethods := methodOrder(t, generate(t, "rust")["mod.rs"],
		regexp.MustCompile(`(?m)^\s{4}(\w+) = (\d+),`))
	goMethods := methodOrder(t, generate(t, "go")["tables.go"],
		regexp.MustCompile(`(?m)^\tMethod(\w+) = (\d+)$`))

	if len(rustMethods) != len(goMethods) {
		t.Fatalf("method counts differ: rust %d, go %d", len(rustMethods), len(goMethods))
	}
	for i := range rustMethods {
		if rustMethods[i] != goMethods[i] {
			t.Errorf("handler %d differs: rust %q, go %q", i, rustMethods[i], goMethods[i])
		}
	}
}

// methodOrder extracts method names in handler-index order, asserting the
// indices are dense and ascending.
func methodOrder(t *testing.T, source string, pattern *regexp.Regexp) []string {
	t.Helper()

	matches := pattern.FindAllStringSubmatch(source, -1)
	if len(matches) == 0 {
		t.Fatal("no method constants found")
	}

	names := make([]string, 0, len(matches))
	for i, match := range matches {
		if want := itoa(i); match[2] != want {
			t.Errorf("method %q has index %s, want %s: indices must be dense and in order",
				match[1], match[2], want)
		}
		names = append(names, match[1])
	}
	return names
}

// itoa renders a small non-negative int, avoiding a strconv import for one use.
func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	var digits []byte
	for n > 0 {
		digits = append([]byte{byte('0' + n%10)}, digits...)
		n /= 10
	}
	return string(digits)
}

func TestTargetsAgreeOnMutability(t *testing.T) {
	// A Mutating selector dispatches on this. The two runtimes classifying one
	// method differently would mean a policy that covers it in Rust and silently
	// skips it in Go.
	rust := generate(t, "rust")["mod.rs"]
	golang := generate(t, "go")["tables.go"]

	for _, method := range []struct {
		name     string
		mutating bool
	}{
		{"GetArtist", false},
		{"ListArtists", false},
		{"CreateArtist", true},
		{"UpdateTrack", true},
		{"DeleteTrack", true},
		// A custom method bound only to GET is read-only.
		{"WatchTracks", false},
	} {
		rustWant := "Method::" + method.name + " => " + boolText(method.mutating)
		if !strings.Contains(rust, rustWant) {
			t.Errorf("mod.rs is missing %q", rustWant)
		}

		goWant := `FullName:     "` + fullNameOf(method.name) + `"`
		block := methodBlock(golang, goWant)
		if block == "" {
			t.Errorf("tables.go has no entry for %s", method.name)
			continue
		}
		if strings.Contains(block, "Mutating:     true") != method.mutating {
			t.Errorf("%s: go says mutating=%v, want %v", method.name,
				!method.mutating, method.mutating)
		}
	}
}

// fullNameOf returns the fully-qualified proto name of an example method.
func fullNameOf(name string) string {
	if strings.HasSuffix(name, "Artist") || strings.HasSuffix(name, "Artists") {
		return "music.v1.ArtistService." + name
	}
	return "music.v1.TrackService." + name
}

// methodBlock returns the emitted method-table entry containing a marker.
func methodBlock(source, marker string) string {
	index := strings.Index(source, marker)
	if index < 0 {
		return ""
	}
	end := strings.Index(source[index:], "},")
	if end < 0 {
		return source[index:]
	}
	return source[index : index+end]
}

// boolText spells a bool as the targets emit it.
func boolText(value bool) string {
	if value {
		return "true"
	}
	return "false"
}
