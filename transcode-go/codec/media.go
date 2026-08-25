package codec

// media.go parses and matches media types.

import "strings"

// MediaType is a parsed media type: a type, a subtype, and the parameters that
// are ignored for matching.
type MediaType struct {
	// Type is the top-level type, e.a. "application". "*" for a wildcard.
	Type string

	// Subtype is the subtype, e.a. "json". "*" for a wildcard.
	Subtype string
}

// ParseMediaType parses one media type, discarding its parameters.
//
// Parameters other than charset carry no meaning for codec selection, per
// README §3, and charset is only ever UTF-8 here: protojson is defined as UTF-8
// and protobuf is binary.
//
// Reports false when the input is not type/subtype.
func ParseMediaType(raw string) (MediaType, bool) {
	value := strings.TrimSpace(strings.Split(raw, ";")[0])
	typ, subtype, found := strings.Cut(value, "/")
	if !found {
		return MediaType{}, false
	}

	typ, subtype = strings.TrimSpace(typ), strings.TrimSpace(subtype)
	if typ == "" || subtype == "" {
		return MediaType{}, false
	}
	return MediaType{Type: typ, Subtype: subtype}, true
}

// Matches reports whether this media type matches a concrete one, honouring
// wildcards on this side only.
//
// The asymmetry is intentional: an Accept entry may be */*, but a codec's
// registered type never is, so wildcards belong to the request.
func (m MediaType) Matches(concrete string) bool {
	other, ok := ParseMediaType(concrete)
	if !ok {
		return false
	}
	typeOK := m.Type == "*" || strings.EqualFold(m.Type, other.Type)
	subtypeOK := m.Subtype == "*" || strings.EqualFold(m.Subtype, other.Subtype)
	return typeOK && subtypeOK
}

// Specificity is how specific the media type is, for RFC 9110 precedence: */*
// is 0, type/* is 1, type/subtype is 2.
//
// A more specific Accept entry outranks a less specific one at the same
// quality, so "Accept: */*, application/json" prefers JSON.
func (m MediaType) Specificity() uint8 {
	switch {
	case m.Type == "*":
		return 0
	case m.Subtype == "*":
		return 1
	}
	return 2
}

// IsAny reports whether this is the */* wildcard, which accepts anything.
func (m MediaType) IsAny() bool { return m.Type == "*" && m.Subtype == "*" }

// String renders the media type as type/subtype.
func (m MediaType) String() string { return m.Type + "/" + m.Subtype }
