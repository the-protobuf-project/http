package music

// requestids.go is the in-memory request-id store the example runs with.

import (
	"sync"

	"github.com/the-protobuf-project/grpc-gateway-rs/examples/music-go/gateway"
)

// Domain returns the API's error domain, which the generated table holds.
//
// A function so the binary and the tests reach it without importing the
// generated package directly, which keeps the example's import graph one layer
// deep.
func Domain() string { return gateway.Domain }

// RequestIDs remembers the AIP-155 request ids already seen.
//
// Per-process and unbounded, which is exactly what a real deployment must not
// use: a retry landing on another replica would execute twice, and nothing here
// ever forgets an id. It is a demonstration of the interface, not an
// implementation to copy.
type RequestIDs struct {
	// mu guards seen.
	mu sync.Mutex

	// seen holds the ids already recorded, keyed by method and id together so
	// two methods cannot deduplicate each other's calls.
	seen map[string]bool
}

// NewRequestIDs returns an empty store.
func NewRequestIDs() *RequestIDs { return &RequestIDs{seen: map[string]bool{}} }

// Record implements [builtin.RequestIDStore].
func (r *RequestIDs) Record(method, requestID string) bool {
	r.mu.Lock()
	defer r.mu.Unlock()

	key := method + "\x00" + requestID
	if r.seen[key] {
		return false
	}
	r.seen[key] = true
	return true
}
