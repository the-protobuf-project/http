package apierr

// duration.go renders a duration the way protojson spells it.

import (
	"strconv"
	"strings"
)

// trimZeros renders seconds with up to nanosecond precision and no trailing
// zeros, which is the form protojson emits: "1.5", "0.000340012", "30".
//
// strconv with 'f' and -1 precision would use the shortest representation that
// round-trips a float64, which for a value like 0.1 is exactly what is wanted
// but for one like 1e-9 becomes scientific notation that the protojson grammar
// does not accept.
func trimZeros(seconds float64) string {
	out := strconv.FormatFloat(seconds, 'f', 9, 64)
	if !strings.Contains(out, ".") {
		return out
	}
	out = strings.TrimRight(out, "0")
	return strings.TrimSuffix(out, ".")
}
