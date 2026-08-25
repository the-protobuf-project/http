package builtin

// ratelimit.go rejects a call that is over quota.

import (
	"fmt"
	"time"

	"github.com/the-protobuf-project/grpc-gateway-rs/netadapter/apierr"
	"github.com/the-protobuf-project/grpc-gateway-rs/netadapter/middleware"
)

// Limiter decides whether a call is within its quota.
//
// The adapter does not implement the counting: a real limit is shared across
// replicas and belongs in Redis or a sidecar, and a per-process token bucket
// would quietly permit N times the configured rate.
type Limiter interface {
	// Allow reports whether this call may proceed, and if not, how long to wait.
	//
	// key identifies the subject — a caller id, an address, a project.
	Allow(key, method string) (retryAfter time.Duration, allowed bool)
}

// RateLimit rejects a call that is over quota.
//
// The 429 carries a QuotaFailure naming the subject and a RetryInfo the error
// model projects to Retry-After, so a client knows both why it was refused and
// when to come back.
type RateLimit struct {
	// limiter decides whether a call is within quota.
	limiter Limiter

	// domain is the API's error domain.
	domain string
}

// NewRateLimit returns the interceptor.
func NewRateLimit(limiter Limiter, domain string) *RateLimit {
	return &RateLimit{limiter: limiter, domain: domain}
}

// Name implements [middleware.Interceptor].
func (*RateLimit) Name() string { return "rate-limit" }

// OnRoute implements [middleware.RouteHook].
func (r *RateLimit) OnRoute(cx *middleware.RouteCx) error {
	key := limitKey(cx)
	retryAfter, allowed := r.limiter.Allow(key, cx.Method.FullName)
	if allowed {
		return nil
	}

	return apierr.New(apierr.ResourceExhausted, "Quota exceeded for this caller.").
		WithErrorInfo("RATE_LIMIT_EXCEEDED", r.domain, map[string]string{
			"method": cx.Method.FullName,
		}).
		WithDetail(apierr.QuotaFailure{Violations: []apierr.QuotaViolation{{
			Subject:     key,
			Description: fmt.Sprintf("Too many requests to %s.", cx.Method.FullName),
		}}}).
		WithDetail(apierr.RetryInfo{RetryDelay: retryAfter})
}

// limitKey is the subject a limit applies to.
//
// The authenticated caller when there is one, otherwise the resolved client
// address, otherwise the method. Preferring identity over address matters: NAT
// puts many callers behind one address, and limiting by address alone punishes
// all of them for one.
func limitKey(cx *middleware.RouteCx) string {
	if identity, ok := IdentityFrom(cx); ok {
		return "sub:" + identity.Subject
	}
	if ip, ok := ClientIPFrom(cx); ok {
		return "ip:" + ip
	}
	return "method:" + cx.Method.FullName
}
