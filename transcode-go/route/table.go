package route

// table.go is the route table itself: what it holds and how a request is
// scanned against it.

// Table is a compiled route table, emitted by protoc-gen-http already sorted
// most-specific-first.
//
// The generator has already rejected any table containing an unresolvable
// ambiguity, so a linear scan in the emitted order is both correct and
// complete: the first route that matches is the one that should serve. This is
// the payoff of deciding precedence at build time — grpc-gateway resolves
// overlapping patterns by registration order, at request time, silently.
type Table struct {
	// routes are the routes, most specific first.
	routes []Route

	// methods are the methods routes dispatch to, in handler-index order.
	methods []Method

	// hasVerbRoutes caches whether any route declares a custom verb, because
	// that decides whether a trailing ":" in a path is worth treating as a verb
	// at all, and that question is asked on every request.
	hasVerbRoutes bool
}

// NewTable builds a table over a generated route slice and its method table.
func NewTable(routes []Route, methods []Method) *Table {
	table := &Table{routes: routes, methods: methods}
	for i := range routes {
		if routes[i].Verb != "" {
			table.hasVerbRoutes = true
			break
		}
	}
	return table
}

// Routes returns the routes in scan order.
func (t *Table) Routes() []Route { return t.routes }

// Methods returns the method table, in handler-index order.
func (t *Table) Methods() []Method { return t.methods }

// Method returns the method a handler index names, and whether the index is one
// the generator emitted.
//
// A missing index means the route table and the method table disagree, which is
// a generator bug rather than anything a caller did — so it is reported rather
// than panicked on.
func (t *Table) Method(handler int) (Method, bool) {
	if handler < 0 || handler >= len(t.methods) {
		return Method{}, false
	}
	return t.methods[handler], true
}

// Resolve resolves an HTTP method and path against the table.
//
// A ":" is legal inside a resource id, so a peeled verb is a guess: the
// verb-bearing routes are tried first, and if none claims it the path is
// retried with the colon as data. A suffix no registered route asked for is
// never stripped.
func (t *Table) Resolve(method, path string) Resolution {
	if t.hasVerbRoutes {
		segments, verb := SplitPath(path, true)
		if verb != "" {
			if resolved := t.scan(method, segments, verb); resolved.Outcome != NotFound {
				return resolved
			}
		}
	}
	segments, _ := SplitPath(path, false)
	return t.scan(method, segments, "")
}

// scan walks the table once, collecting the methods bound to a matching path so
// a 405 can name them.
func (t *Table) scan(method string, segments []string, verb string) Resolution {
	var allow []string

	for i := range t.routes {
		route := &t.routes[i]
		if !route.Matches(segments, verb) {
			continue
		}
		if route.Method == method {
			return Resolution{
				Outcome:  Matched,
				Route:    route,
				Segments: segments,
				Verb:     verb,
			}
		}
		if !contains(allow, route.Method) {
			allow = append(allow, route.Method)
		}
	}

	if len(allow) == 0 {
		return Resolution{Outcome: NotFound, Segments: segments, Verb: verb}
	}
	return Resolution{Outcome: MethodNotAllowed, Segments: segments, Verb: verb, Allow: allow}
}

// contains reports whether a short slice already holds a value. A map would
// cost an allocation per request to save a scan over at most a handful of
// methods.
func contains(values []string, want string) bool {
	for _, value := range values {
		if value == want {
			return true
		}
	}
	return false
}
