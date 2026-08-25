package middleware

// selector.go chooses which methods an interceptor applies to.

import "github.com/the-protobuf-project/http/transcode-go/route"

// Selector decides which methods an interceptor applies to.
//
// Borrowed from go-grpc-middleware, and stronger here because the IR knows what
// methods mean: [Mutating] covers every Create, Update, Delete and Undelete, so
// adding one later is covered automatically. A policy written against a name
// prefix would silently miss it.
type Selector func(route.Method) bool

// All selects every method.
func All() Selector { return func(route.Method) bool { return true } }

// Mutating selects the methods that change state.
func Mutating() Selector {
	return func(m route.Method) bool { return m.Mutating }
}

// ReadOnly selects the methods that only read.
func ReadOnly() Selector {
	return func(m route.Method) bool { return !m.Mutating }
}

// Pattern selects the methods with one AIP classification.
func Pattern(want route.Pattern) Selector {
	return func(m route.Method) bool { return m.Pattern == want }
}

// Service selects every method of one service, by fully-qualified name.
func Service(name string) Selector {
	return func(m route.Method) bool { return m.Service == name }
}

// Method selects one method, by fully-qualified name.
func Method(name string) Selector {
	return func(m route.Method) bool { return m.FullName == name }
}

// Streaming selects the methods that stream their response.
func Streaming() Selector {
	return func(m route.Method) bool { return m.ServerStream }
}

// Not selects every method the inner selector does not.
func Not(inner Selector) Selector {
	return func(m route.Method) bool { return !inner(m) }
}

// Any selects a method matched by at least one inner selector.
func Any(inner ...Selector) Selector {
	return func(m route.Method) bool {
		for _, selector := range inner {
			if selector(m) {
				return true
			}
		}
		return false
	}
}

// Every selects a method matched by all of the inner selectors.
//
// Named Every rather than All because All already means "every method", and a
// stack reading `All()` and `All(a, b)` differently would be a trap.
func Every(inner ...Selector) Selector {
	return func(m route.Method) bool {
		for _, selector := range inner {
			if !selector(m) {
				return false
			}
		}
		return true
	}
}
