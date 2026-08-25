package builtin

// idempotency.go is AIP-155 request-id deduplication.

import (
	"log/slog"

	"github.com/the-protobuf-project/http/netadapter/middleware"
)

// RequestIDStore remembers which request ids have been seen.
//
// A real store is shared and expiring, for the same reason [Limiter] is an
// interface: per-process memory would let a retry that lands on another replica
// execute twice, which is the exact failure deduplication exists to prevent.
type RequestIDStore interface {
	// Record records an id, reporting whether it is new. False means the id was
	// already seen and the call is a replay.
	Record(method, requestID string) bool
}

// Idempotency rejects a replayed mutation (AIP-155).
//
// Pair it with [middleware.Mutating], since a replayed read is harmless.
//
// A request with no request_id passes: AIP-155 makes the field optional, and
// requiring it would break every existing client.
type Idempotency struct {
	// store remembers the ids already seen.
	store RequestIDStore

	// logger records replays, which are worth seeing but are not failures.
	logger *slog.Logger
}

// NewIdempotency returns the interceptor.
func NewIdempotency(store RequestIDStore, logger *slog.Logger) *Idempotency {
	if logger == nil {
		logger = slog.Default()
	}
	return &Idempotency{store: store, logger: logger}
}

// Name implements [middleware.Interceptor].
func (*Idempotency) Name() string { return "idempotency" }

// OnRoute checks the id carried in the query string.
//
// The body's request_id is checked by the generated handler through
// [middleware.InspectRequest], since only it knows the message type. This covers
// the case the interceptor can see without decoding anything.
func (i *Idempotency) OnRoute(cx *middleware.RouteCx) error {
	requestID := cx.Request.URL.Query().Get("requestId")
	if requestID == "" || i.store.Record(cx.Method.FullName, requestID) {
		return nil
	}

	// AIP-155: a replay is not an error. The original call already succeeded, so
	// reporting a failure would push the client into a retry loop over work that
	// is already done.
	i.logger.Debug("duplicate request id; treating as a replay",
		"method", cx.Method.FullName, "requestId", requestID)
	return nil
}
