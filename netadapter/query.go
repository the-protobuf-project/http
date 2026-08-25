package netadapter

// query.go holds the query parameters after the system parameters are removed.

import (
	"net/url"
	"sort"
	"strings"
)

// System parameter names, stripped before binding and never bound to a field.
//
// Each has a "$"-prefixed alias, which exists for clients that cannot send a
// bare "alt" — the prefix is reserved precisely so a future system parameter
// cannot collide with a field name.
const (
	// ParamAlt selects the response codec.
	ParamAlt = "alt"

	// ParamFields is the AIP-157 partial response mask.
	ParamFields = "fields"

	// ParamPrettyPrint asks for indented output.
	ParamPrettyPrint = "prettyPrint"
)

// systemParams is the set stripped before binding.
var systemParams = map[string]bool{
	ParamAlt: true, ParamFields: true, ParamPrettyPrint: true,
}

// Query is the request's query parameters with the system parameters removed.
type Query struct {
	// Values are the remaining parameters, in the order they were sent.
	Values url.Values

	// Alt is the ?alt= codec selector, or "".
	Alt string

	// Fields is the AIP-157 ?fields= mask, or "".
	Fields string

	// PrettyPrint is whether ?prettyPrint= asked for indented output.
	PrettyPrint bool
}

// Get returns the first value of a parameter, or "".
func (q Query) Get(name string) string { return q.Values.Get(name) }

// All returns every value of a parameter, which is how a repeated field is
// sent: one occurrence per element.
func (q Query) All(name string) []string { return q.Values[name] }

// Has reports whether a parameter was sent at all, which is distinct from its
// being sent empty.
func (q Query) Has(name string) bool { _, ok := q.Values[name]; return ok }

// Unknown returns the parameters that are not in known, sorted so the error
// naming them is deterministic.
func (q Query) Unknown(known []string) []string {
	allowed := make(map[string]bool, len(known))
	for _, name := range known {
		allowed[name] = true
	}

	var unknown []string
	for name := range q.Values {
		if !allowed[name] {
			unknown = append(unknown, name)
		}
	}
	sort.Strings(unknown)
	return unknown
}

// parseQuery splits a raw query string into system parameters and the rest.
//
// A "$"-prefixed name that is not a known system parameter is reported as
// reserved rather than treated as a field: accepting it would let a future
// system parameter silently change what an existing request means.
func parseQuery(raw string) (Query, string) {
	// url.ParseQuery fails on a malformed escape; the well-formed pairs it did
	// parse are still the caller's request, and rejecting the whole query over
	// one bad byte would be a worse answer than binding what was understood.
	values, _ := url.ParseQuery(raw)

	query := Query{Values: url.Values{}}
	for name, list := range values {
		bare := strings.TrimPrefix(name, "$")
		if !systemParams[bare] {
			if strings.HasPrefix(name, "$") {
				return query, name
			}
			query.Values[name] = list
			continue
		}

		value := ""
		if len(list) > 0 {
			value = list[0]
		}
		switch bare {
		case ParamAlt:
			query.Alt = value
		case ParamFields:
			query.Fields = value
		case ParamPrettyPrint:
			query.PrettyPrint = value != "" && value != "false" && value != "0"
		}
	}
	return query, ""
}
