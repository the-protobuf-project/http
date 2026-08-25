package builtin

// recovery.go records a panic that the transcoder caught.

import (
	"log/slog"

	"github.com/the-protobuf-project/http/transcode-go/middleware"
)

// Recovery records a panic as a completed call.
//
// A panic in one handler must not take down the connection, because on HTTP/2
// that connection is carrying other people's requests. It also must not reach
// the client: a panic value frequently contains a file path, a slice index, or
// a fragment of the data that caused it.
//
// The catching itself happens in the transcoder, which is the only thing that owns
// the call — an interceptor cannot recover from a panic in a function it never
// called. This interceptor is what makes the panic visible to the same
// observability stack as everything else, rather than only in the transcoder's own
// logger.
type Recovery struct {
	// logger receives the record.
	logger *slog.Logger
}

// NewRecovery returns the interceptor.
func NewRecovery(logger *slog.Logger) *Recovery {
	if logger == nil {
		logger = slog.Default()
	}
	return &Recovery{logger: logger}
}

// Name implements [middleware.Interceptor].
func (*Recovery) Name() string { return "recovery" }

// OnComplete records a call that ended in GATEWAY_PANIC.
func (r *Recovery) OnComplete(cx *middleware.CallCx, outcome middleware.Outcome) {
	if !outcome.Failed() {
		return
	}
	info := outcome.Err.ErrorInfo()
	if info == nil || info.Reason != "GATEWAY_PANIC" {
		return
	}

	r.logger.Error("handler panicked",
		"method", cx.Method.FullName,
		"template", cx.Template,
		"elapsedMs", cx.Elapsed().Milliseconds(),
	)
}
