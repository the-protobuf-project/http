package codec

// accept.go parses the Accept header and orders its entries by preference.

import (
	"sort"
	"strings"
)

// AcceptEntry is one entry of an Accept header: a media range and its quality.
type AcceptEntry struct {
	// Media is the media range, which may contain wildcards.
	Media MediaType

	// Quality is the quality value, scaled to thousandths so entries sort as
	// integers. RFC 9110 allows at most three decimal places, so this is exact
	// rather than a rounding of the float the header actually spells.
	Quality uint16
}

// IsRefusal reports whether the client has explicitly refused this range.
//
// q=0 is a refusal, not a low preference: a codec matched only by a
// zero-quality entry must not be selected.
func (e AcceptEntry) IsRefusal() bool { return e.Quality == 0 }

// ParseAccept parses an Accept header into entries ordered by preference, most
// preferred first.
//
// Ordering follows RFC 9110: quality descending, then specificity descending,
// so "Accept: */*, application/json" prefers JSON even though the wildcard came
// first. Ties keep header order, which is the only tiebreak a client can
// actually control.
//
// Unparseable entries are skipped rather than failing the request — a header
// with one malformed range and one good one should still work.
func ParseAccept(header string) []AcceptEntry {
	var entries []AcceptEntry
	for _, part := range strings.Split(header, ",") {
		part = strings.TrimSpace(part)
		media, ok := ParseMediaType(part)
		if !ok {
			continue
		}
		entries = append(entries, AcceptEntry{Media: media, Quality: parseQuality(part)})
	}

	// SliceStable so equal entries keep the order the client sent.
	sort.SliceStable(entries, func(i, j int) bool {
		if entries[i].Quality != entries[j].Quality {
			return entries[i].Quality > entries[j].Quality
		}
		return entries[i].Media.Specificity() > entries[j].Media.Specificity()
	})
	return entries
}

// AcceptsAnything reports whether an Accept header contains a non-refused */*.
func AcceptsAnything(header string) bool {
	for _, entry := range ParseAccept(header) {
		if !entry.IsRefusal() && entry.Media.IsAny() {
			return true
		}
	}
	return false
}

// parseQuality extracts the q= parameter from one Accept entry, defaulting to
// 1.0. Returns thousandths: q=0.9 is 900, an absent parameter is 1000.
func parseQuality(entry string) uint16 {
	parts := strings.Split(entry, ";")
	for _, param := range parts[1:] {
		key, value, found := strings.Cut(param, "=")
		if !found || !strings.EqualFold(strings.TrimSpace(key), "q") {
			continue
		}
		return parseQValue(strings.TrimSpace(value))
	}
	return 1000
}

// parseQValue parses a quality value into thousandths.
//
// Done by hand rather than through a float so q=0.001 and q=1.0 are exact, and
// so a malformed value degrades to "fully acceptable" rather than to a silent
// refusal — the safer direction when the header is ambiguous.
func parseQValue(raw string) uint16 {
	whole, frac, _ := strings.Cut(raw, ".")
	if strings.TrimSpace(whole) != "0" {
		// "1", and anything unparseable, are both fully acceptable. A fraction
		// after "1" cannot raise it further.
		return 1000
	}

	// At most three digits are significant; anything beyond is truncated.
	scales := []uint16{100, 10, 1}
	var thousandths uint16
	for i, c := range frac {
		if i >= len(scales) {
			break
		}
		if c < '0' || c > '9' {
			return 1000
		}
		thousandths += uint16(c-'0') * scales[i]
	}
	return thousandths
}
