package table

// dedup.go assigns one identifier per distinct value, so bindings that share a
// path shape or a capture set share the emitted array rather than each carrying
// a copy.

import "fmt"

// Dedup maps distinct keys to identifiers, keeping first-seen order so the
// emitted file is stable across runs.
//
// First-seen order rather than sorted order because the caller walks the routes
// in scan order: the arrays then appear in the file roughly in the order the
// table refers to them, which is what a reader following a route to its
// segments wants.
type Dedup struct {
	// order is the identifiers in first-seen order.
	order []string

	// byKey maps a value's key to the identifier assigned to it.
	byKey map[string]string

	// used records every assigned identifier, so a collision between two
	// distinct values is resolved rather than silently shared.
	used map[string]bool
}

// NewDedup returns an empty deduplicator.
func NewDedup() *Dedup {
	return &Dedup{byKey: map[string]string{}, used: map[string]bool{}}
}

// Add returns the identifier for a key, assigning ident the first time the key
// is seen.
//
// A name collision between two *distinct* keys takes a numeric suffix. That is
// a fallback, not the naming scheme: a target whose identifiers routinely
// collide is naming them badly, and the suffix makes the output correct while
// leaving the poor names visible.
func (d *Dedup) Add(key, ident string) string {
	if existing, ok := d.byKey[key]; ok {
		return existing
	}

	base := ident
	for suffix := 2; d.used[ident]; suffix++ {
		ident = fmt.Sprintf("%s_%d", base, suffix)
	}
	d.used[ident] = true
	d.byKey[key] = ident
	d.order = append(d.order, ident)
	return ident
}

// Ident returns the identifier assigned to a key, or "" when the key is unseen.
func (d *Dedup) Ident(key string) string { return d.byKey[key] }

// Order returns the assigned identifiers in first-seen order.
func (d *Dedup) Order() []string { return d.order }
