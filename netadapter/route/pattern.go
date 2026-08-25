package route

// pattern.go classifies a method against the AIP standard methods.

// Pattern is how a method is classified against the AIP standard methods.
//
// Emitted by protoc-gen-http from the method's name and its google.api.http
// rule, which is what makes a policy expressible in terms of what a method
// means rather than what it is called.
type Pattern uint8

const (
	// PatternCustom is AIP-136: anything that is not a standard method. The
	// zero value, because it is the honest answer for an unannotated method.
	PatternCustom Pattern = iota

	// PatternGet is AIP-131.
	PatternGet

	// PatternList is AIP-132.
	PatternList

	// PatternCreate is AIP-133.
	PatternCreate

	// PatternUpdate is AIP-134.
	PatternUpdate

	// PatternDelete is AIP-135.
	PatternDelete

	// PatternUndelete is AIP-164.
	PatternUndelete

	// PatternBatchGet is AIP-231.
	PatternBatchGet

	// PatternBatchCreate is AIP-233.
	PatternBatchCreate

	// PatternBatchUpdate is AIP-234.
	PatternBatchUpdate

	// PatternBatchDelete is AIP-235.
	PatternBatchDelete
)

// patternNames is the spelling used in metric labels and diagnostics.
var patternNames = map[Pattern]string{
	PatternCustom: "Custom", PatternGet: "Get", PatternList: "List",
	PatternCreate: "Create", PatternUpdate: "Update", PatternDelete: "Delete",
	PatternUndelete: "Undelete", PatternBatchGet: "BatchGet",
	PatternBatchCreate: "BatchCreate", PatternBatchUpdate: "BatchUpdate",
	PatternBatchDelete: "BatchDelete",
}

// String names the pattern.
func (p Pattern) String() string {
	if name, ok := patternNames[p]; ok {
		return name
	}
	return "Custom"
}

// Mutating reports whether the pattern changes state.
//
// A custom method counts as mutating: it is the conservative reading, and a
// read-only custom method that wants otherwise can be named explicitly in a
// selector.
func (p Pattern) Mutating() bool {
	switch p {
	case PatternGet, PatternList, PatternBatchGet:
		return false
	}
	return true
}

// ReadOnly reports whether the pattern only reads.
func (p Pattern) ReadOnly() bool { return !p.Mutating() }
