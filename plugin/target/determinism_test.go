package target_test

// determinism_test.go asserts that generating twice produces identical bytes.
//
// This catches the bug a committed golden file cannot: a map ranged into output
// produces a different order on some runs and the same order on others, so a
// golden file regenerated on a lucky run locks in a file that the next run
// disagrees with. Every target is covered, including ones added later.

import "testing"

func TestEveryTargetIsDeterministic(t *testing.T) {
	for _, lang := range languages(t) {
		t.Run(lang, func(t *testing.T) {
			first := generate(t, lang)
			second := generate(t, lang)

			if len(first) != len(second) {
				t.Fatalf("file counts differ: %d then %d", len(first), len(second))
			}
			for name, content := range first {
				other, ok := second[name]
				if !ok {
					t.Errorf("%s was emitted once and not the second time", name)
					continue
				}
				if content != other {
					t.Errorf("%s differs between runs; something iterated a map into the output", name)
				}
			}
		})
	}
}

func TestEveryTargetStampsTheDomain(t *testing.T) {
	// The domain is not derivable from the protos, so a target that dropped it
	// would emit a gateway whose every ErrorInfo names an empty domain — which
	// is well-formed, and useless.
	for _, lang := range languages(t) {
		t.Run(lang, func(t *testing.T) {
			var found bool
			for _, content := range generate(t, lang) {
				if contains(content, testDomain) {
					found = true
				}
			}
			if !found {
				t.Errorf("no emitted file carries the error domain %q", testDomain)
			}
		})
	}
}

// contains reports whether a haystack holds a needle.
func contains(haystack, needle string) bool {
	return len(haystack) >= len(needle) && indexOf(haystack, needle) >= 0
}

// indexOf is strings.Index, inlined so this file's intent stays one import
// shallower than the assertion it makes.
func indexOf(haystack, needle string) int {
	for i := 0; i+len(needle) <= len(haystack); i++ {
		if haystack[i:i+len(needle)] == needle {
			return i
		}
	}
	return -1
}
