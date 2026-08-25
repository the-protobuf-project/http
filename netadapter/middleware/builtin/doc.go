// Package builtin holds the interceptors that ship with the adapter.
//
// The set mirrors go-grpc-middleware's, plus the two grpc-gateway offers as mux
// options ([Health] and [CORS]):
//
//	Recovery     catches a panic, emits 500 / GATEWAY_PANIC, keeps the connection
//	Deadline     Grpc-Timeout → RPC deadline, capped, with a mandatory default
//	Auth         pluggable verifier, 401 with a well-formed WWW-Authenticate
//	RateLimit    429 with QuotaFailure + RetryInfo + Retry-After
//	RealIP       resolves the client behind N trusted proxies
//	Validate     rejects a request that fails a generated rule
//	Idempotency  AIP-155 request_id deduplication
//	Logging      one structured line per call, labelled by template
//	Metrics      bounded-cardinality metrics through a sink interface
//	Health       a liveness endpoint, as WithHealthzEndpoint provides
//	CORS         preflight and headers, Allow-Methods exact from the route table
//
// retry has no counterpart, deliberately. In go-grpc-middleware it is a client
// interceptor, and retrying at the adapter would be wrong: the adapter cannot
// know whether a method is idempotent, and replaying a non-idempotent one turns
// a timeout into a duplicate write.
//
// [RateLimit] and [Idempotency] take interfaces rather than implementations,
// because a per-process token bucket silently permits N times the configured
// rate across replicas, and a per-process request-id set lets a retry landing on
// another replica execute twice.
package builtin
