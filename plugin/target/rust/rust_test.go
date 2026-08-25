package rust

import (
	"strings"
	"testing"
)

func TestEmitsTheThreeModuleFiles(t *testing.T) {
	files := generate(t)
	for _, name := range []string{"mod.rs", "matches.rs", "routes.rs"} {
		if _, ok := files[name]; !ok {
			t.Errorf("no %s emitted; got %v", name, keys(files))
		}
	}
}

func TestMatchIdentifiersSpellThePathShape(t *testing.T) {
	matches := generate(t)["matches.rs"]

	// `/v1/artists` and `/v1/artists/*` pin the same literal, so naming by
	// literals alone would collide and leave a reader with M_ARTISTS and
	// M_ARTISTS_2, neither of which says which is which.
	for _, want := range []string{
		"M_ARTISTS:",
		"M_ARTISTS_ANY:",
		"M_ARTISTS_ANY_TRACKS:",
		"M_ARTISTS_ANY_TRACKS_ANY:",
	} {
		if !strings.Contains(matches, want) {
			t.Errorf("matches.rs is missing %q", want)
		}
	}
	if strings.Contains(matches, "M_ARTISTS_2") {
		t.Error("matches.rs has a collision-suffixed identifier; the shape naming failed")
	}
}

func TestRoutesAreSortedMostSpecificFirst(t *testing.T) {
	routes := generate(t)["routes.rs"]

	// A custom verb is an extra constraint, so a verb-bearing route must
	// precede its verbless twin; otherwise the twin would swallow it.
	withdraw := strings.Index(routes, "Method::WithdrawTrack")
	getTrack := strings.Index(routes, "Method::GetTrack")
	if withdraw < 0 || getTrack < 0 {
		t.Fatal("routes.rs is missing WithdrawTrack or GetTrack")
	}
	if withdraw > getTrack {
		t.Error("the :withdraw route must precede the verbless track route")
	}
}

func TestEveryBindingReachesTheTable(t *testing.T) {
	routes := generate(t)["routes.rs"]

	// Twelve methods, one binding each.
	if got := strings.Count(routes, "    route("); got != 12 {
		t.Errorf("route count = %d, want 12", got)
	}
	for _, template := range []string{
		"/v1/artists",
		"/v1/{name=artists/*}",
		"/v1/{artist.name=artists/*}",
		"/v1/{name=artists/*/tracks/*}",
		"/v1/{parent=artists/*}/tracks",
		"/v1/{name=artists/*/tracks/*}:withdraw",
		"/v1/{parent=artists/*}/tracks:watch",
	} {
		if !strings.Contains(routes, `"`+template+`"`) {
			t.Errorf("routes.rs is missing the template %q", template)
		}
	}
}

func TestMethodEnumCarriesAIPDerivedMutability(t *testing.T) {
	mod := generate(t)["mod.rs"]

	// A Selector::Mutating policy dispatches on this, so it has to come from
	// the AIP pattern rather than from a name prefix.
	for _, want := range []string{
		"Method::GetArtist => false",
		"Method::CreateArtist => true",
		"Method::DeleteTrack => true",
		// A custom method bound only to GET is read-only.
		"Method::WatchTracks => false",
	} {
		if !strings.Contains(mod, want) {
			t.Errorf("mod.rs is missing %q", want)
		}
	}
}

func TestTheDomainIsStamped(t *testing.T) {
	mod := generate(t)["mod.rs"]
	if !strings.Contains(mod, `pub const DOMAIN: &str = "music.example.com"`) {
		t.Error("mod.rs does not carry the configured error domain")
	}
}

func TestGenerationIsDeterministic(t *testing.T) {
	// The output is committed, so a build that reorders it would produce a
	// diff on every run and make a real change impossible to spot.
	first, second := generate(t), generate(t)
	for name, content := range first {
		if second[name] != content {
			t.Errorf("%s differs between two runs", name)
		}
	}
}

// keys returns a map's keys, for an error message.
func keys(m map[string]string) []string {
	out := make([]string, 0, len(m))
	for name := range m {
		out = append(out, name)
	}
	return out
}
