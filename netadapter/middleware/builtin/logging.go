package builtin

// logging.go emits one structured line per completed call.

import (
	"log/slog"
	"time"

	"github.com/the-protobuf-project/http/netadapter/middleware"
)

// defaultSlowThreshold is the latency above which a successful call is worth
// noticing.
const defaultSlowThreshold = time.Second

// Logging logs one line per completed call.
//
// Labels use the template rather than the concrete path, so a log aggregator
// groups /v1/artists/miles and /v1/artists/coltrane under /v1/{name=artists/*}
// instead of treating every resource name as its own event.
//
// Nothing from the request body or the query string is logged. Those carry
// caller data, and a log line is exactly the wrong place for it to end up.
type Logging struct {
	// logger receives the lines.
	logger *slog.Logger

	// slowAfter is the latency above which a success is logged as slow.
	slowAfter time.Duration
}

// NewLogging logs every call, warning on those over a second.
func NewLogging(logger *slog.Logger) *Logging {
	if logger == nil {
		logger = slog.Default()
	}
	return &Logging{logger: logger, slowAfter: defaultSlowThreshold}
}

// SlowAfter sets the latency above which a successful call is logged as slow.
func (l *Logging) SlowAfter(threshold time.Duration) *Logging {
	l.slowAfter = threshold
	return l
}

// Name implements [middleware.Interceptor].
func (*Logging) Name() string { return "logging" }

// OnComplete emits the access log line.
//
// The level follows what a reader can act on: a 5xx is the service's problem, a
// slow success is worth noticing, and everything else is routine.
func (l *Logging) OnComplete(cx *middleware.CallCx, outcome middleware.Outcome) {
	elapsed := cx.Elapsed()
	attrs := []any{
		"method", cx.Method.FullName,
		"template", cx.Template,
		"httpMethod", cx.Request.Method,
		"status", outcome.Status,
		"code", outcome.Code(),
		"elapsedMs", elapsed.Milliseconds(),
	}

	switch {
	case outcome.Failed() && outcome.Status >= 500:
		l.logger.Error("request failed", append(attrs, "message", outcome.Err.Message)...)
	case outcome.Failed():
		l.logger.Info("request rejected", attrs...)
	case elapsed >= l.slowAfter:
		l.logger.Warn("request completed slowly", attrs...)
	default:
		l.logger.Info("request completed", attrs...)
	}
}
