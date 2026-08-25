// Package table holds the half of a generated route table that is the same in
// every language.
//
// A target's job is to render: how a literal is spelled, what an identifier
// looks like, which file the output lands in. Everything before that — which
// bindings exist, what order they scan in, which handler index a method gets,
// which match sequences can be shared — is a property of the API, not of the
// language, and belongs in one place.
//
// Keeping it here is not tidiness. The Rust, Go and Python tables are only
// trustworthy if they describe the same API in the same order; three copies of
// the sort would be three chances to disagree, and nothing in a per-language
// golden test would catch a divergence between two of them.
package table

import (
	"sort"

	"github.com/the-protobuf-project/protokit/service"
	"github.com/the-protobuf-project/protokit/service/httprule"
)

// Entry is one HTTP binding together with the method and service it was
// declared on.
//
// The IR nests bindings under methods under services; a route table is flat,
// because a request is matched before anything knows which service will serve
// it. Entry is that flattening.
type Entry struct {
	// Service is the service the binding's method belongs to.
	Service *service.Service

	// Method is the RPC the binding is declared on.
	Method *service.Method

	// Binding is the google.api.http rule itself.
	Binding *service.Binding
}

// Entries returns every binding in the IR, in declaration order.
//
// Declaration order is not scan order — [Sort] produces that — but it is stable
// and it is what the method index is built from, so a route table that reorders
// itself cannot renumber the handlers underneath it.
func Entries(ir *service.IR) []Entry {
	var entries []Entry
	for _, svc := range ir.Services {
		for _, method := range svc.Methods {
			for _, binding := range method.Bindings {
				entries = append(entries, Entry{
					Service: svc,
					Method:  method,
					Binding: binding,
				})
			}
		}
	}
	return entries
}

// Sort orders entries most specific first, which is the order a runtime scans
// them in.
//
// The generator has already rejected any table with an unresolvable ambiguity,
// so a linear scan in this order is both correct and complete: the first route
// that matches is the one that should serve. That is the whole payoff of
// deciding precedence at build time rather than by registration order at
// request time.
//
// Ties break on the binding's source name so the emitted table is byte
// identical across runs — a map iterated into output is the classic way a
// generator stops being reproducible, and a committed golden file cannot catch
// it.
func Sort(entries []Entry) {
	sort.SliceStable(entries, func(i, j int) bool {
		a, b := entries[i].Binding.Route, entries[j].Binding.Route
		if c := httprule.Compare(a, b); c != 0 {
			return c < 0
		}
		return a.Source < b.Source
	})
}
