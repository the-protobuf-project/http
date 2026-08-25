package route

// split.go segments a request path and peels an AIP-136 custom verb.

import "strings"

// SplitPath splits a request path into raw segments and peels a custom verb.
//
// Splitting happens on the undecoded path, per README §1.2 step 2, so a %2F
// cannot create a segment boundary.
//
// The verb is only separated when hasVerbRoutes says some registered route
// declares one. A ":" is legal inside a resource id, so stripping a suffix
// nobody asked for would bind the id to the wrong value — which is exactly the
// failure mode of feeding /v1/{name}:cancel to a general-purpose router, which
// accepts it and silently folds ":cancel" into name.
//
// Returns the raw segments and the verb without its colon, or "".
func SplitPath(path string, hasVerbRoutes bool) ([]string, string) {
	path = strings.TrimPrefix(path, "/")
	segments := strings.Split(path, "/")
	verb := ""

	if hasVerbRoutes && len(segments) > 0 {
		last := segments[len(segments)-1]
		if idx := strings.LastIndex(last, ":"); idx >= 0 {
			head, tail := last[:idx], last[idx:]
			// Both halves must be non-empty: ":x" has no resource id and "x:"
			// has no verb, and neither is a custom method.
			if head != "" && len(tail) > 1 {
				verb = tail[1:]
				segments[len(segments)-1] = head
			}
		}
	}
	return segments, verb
}
