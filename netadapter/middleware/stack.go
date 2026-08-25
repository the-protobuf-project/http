package middleware

// stack.go assembles interceptors and resolves them against methods.

import (
	"fmt"

	"github.com/the-protobuf-project/http/netadapter/route"
)

// entry is one registered interceptor and the methods it applies to.
type entry struct {
	// interceptor is the policy.
	interceptor Interceptor

	// selector decides which methods it runs for.
	selector Selector
}

// Stack is the ordered set of interceptors an adapter runs.
//
// Order is registration order in every phase, including the response phases.
// Reversing on the way out would be the wrapper convention, and it is wrong
// here: these are policies, not wrappers, and a reader tracing an audit log
// should not have to invert the list to work out what ran when.
type Stack struct {
	// entries are the registered interceptors, in registration order.
	entries []entry
}

// NewStack returns an empty stack.
func NewStack() *Stack { return &Stack{} }

// Use registers an interceptor for every method.
func (s *Stack) Use(interceptor Interceptor) *Stack {
	return s.UseFor(interceptor, All())
}

// UseFor registers an interceptor for the methods a selector matches.
//
// It panics on an interceptor that implements none of the phase interfaces:
// that is a policy which can never run, and the mistake is silent otherwise —
// a misspelled OnRoute compiles fine and simply does nothing.
func (s *Stack) UseFor(interceptor Interceptor, selector Selector) *Stack {
	if !hasAnyPhase(interceptor) {
		panic(fmt.Sprintf(
			"middleware: %s implements no phase; it would never run", interceptor.Name(),
		))
	}
	s.entries = append(s.entries, entry{interceptor: interceptor, selector: selector})
	return s
}

// hasAnyPhase reports whether an interceptor implements at least one phase.
func hasAnyPhase(interceptor Interceptor) bool {
	switch interceptor.(type) {
	case RouteHook, RequestHook, ResponseHook, CompleteHook:
		return true
	}
	return false
}

// Selected is the interceptors that apply to one method, grouped by phase.
//
// Resolved once per method when the adapter is built, not per request: a
// selector is a predicate over the method table, and the method table is fixed
// at generation time.
type Selected struct {
	// Route are the hooks that run after routing.
	Route []RouteHook

	// Request are the hooks that run before the RPC.
	Request []RequestHook

	// Response are the hooks that run before the response is encoded.
	Response []ResponseHook

	// Complete are the hooks that run after everything.
	Complete []CompleteHook
}

// Empty reports whether no interceptor applies, which lets a caller skip the
// phase machinery entirely.
func (s Selected) Empty() bool {
	return len(s.Route)+len(s.Request)+len(s.Response)+len(s.Complete) == 0
}

// For resolves the stack against one method.
func (s *Stack) For(method route.Method) Selected {
	var selected Selected
	for _, e := range s.entries {
		if !e.selector(method) {
			continue
		}
		if hook, ok := e.interceptor.(RouteHook); ok {
			selected.Route = append(selected.Route, hook)
		}
		if hook, ok := e.interceptor.(RequestHook); ok {
			selected.Request = append(selected.Request, hook)
		}
		if hook, ok := e.interceptor.(ResponseHook); ok {
			selected.Response = append(selected.Response, hook)
		}
		if hook, ok := e.interceptor.(CompleteHook); ok {
			selected.Complete = append(selected.Complete, hook)
		}
	}
	return selected
}

// Names returns the registered interceptor names, in order, for a diagnostic.
func (s *Stack) Names() []string {
	names := make([]string, 0, len(s.entries))
	for _, e := range s.entries {
		names = append(names, e.interceptor.Name())
	}
	return names
}
