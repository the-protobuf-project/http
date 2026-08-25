package transcode

// phases.go runs the middleware stack around a call.

import (
	"net/http"
	"time"

	"github.com/the-protobuf-project/http/transcode-go/apierr"
	"github.com/the-protobuf-project/http/transcode-go/middleware"
	"github.com/the-protobuf-project/http/transcode-go/route"
)

// newRouteCx builds the context the route phase sees.
func (h *Handler) newRouteCx(r *http.Request, method route.Method, matched *route.Route, captures map[string]string) *middleware.RouteCx {
	return &middleware.RouteCx{
		Ctx:      r.Context(),
		Request:  r,
		Method:   method,
		Template: matched.Template,
		Captures: captures,
		Peer:     r.RemoteAddr,
		Metadata: middleware.MetadataFromHeaders(r.Header, h.headers.Incoming),
	}
}

// runRoute runs the route phase, and the metadata annotators alongside it.
//
// Annotators run first so a route hook sees the metadata a deployment added:
// an authorizer keyed on a header a WithMetadata equivalent normalised would
// otherwise see nothing.
func (h *Handler) runRoute(cx *middleware.RouteCx, selected middleware.Selected) error {
	for _, annotator := range h.annotators {
		annotator.Annotate(cx.Request.Header, &cx.Metadata)
	}
	for _, hook := range selected.Route {
		if err := hook.OnRoute(cx); err != nil {
			return err
		}
	}
	return nil
}

// runRequest runs the request phase, after the message is bound.
func runRequest(cx *middleware.CallCx, selected middleware.Selected) error {
	for _, hook := range selected.Request {
		if err := hook.OnRequest(cx); err != nil {
			return err
		}
	}
	return nil
}

// runResponse runs the response phase and applies the result to a reply.
//
// The parts start from the reply the handler produced rather than from an empty
// 200, so a hook sees the status the handler actually chose — a 201 from an
// AIP-133 Create, say — and can act on it instead of overwriting it blindly.
func runResponse(cx *middleware.CallCx, selected middleware.Selected, reply *Reply) error {
	if len(selected.Response) == 0 {
		return nil
	}

	parts := &middleware.ResponseParts{
		Status:  reply.Status,
		Header:  reply.Header,
		Trailer: http.Header{},
	}
	for _, hook := range selected.Response {
		if err := hook.OnResponse(cx, parts); err != nil {
			return err
		}
	}

	reply.Status = parts.Status
	reply.Header = parts.Header
	for name, values := range parts.Trailer {
		for _, value := range values {
			reply.Header.Add(http.TrailerPrefix+name, value)
		}
	}
	return nil
}

// runComplete runs the completion phase.
//
// It cannot fail: the response has already been written, so there is nothing an
// error could change. A hook that panics is contained here rather than taking
// down a request that already succeeded — logging and metrics are the last
// things that should be able to fail a call.
func (h *Handler) runComplete(cx *middleware.CallCx, selected middleware.Selected, outcome middleware.Outcome) {
	for _, hook := range selected.Complete {
		h.safeComplete(hook, cx, outcome)
	}
}

// safeComplete runs one completion hook, containing a panic in it.
func (h *Handler) safeComplete(hook middleware.CompleteHook, cx *middleware.CallCx, outcome middleware.Outcome) {
	defer func() {
		if recovered := recover(); recovered != nil {
			h.options.Logger.Error("completion hook panicked",
				"interceptor", hook.Name(), "panic", recovered)
		}
	}()
	hook.OnComplete(cx, outcome)
}

// newCallCx promotes a routing context to a call context.
//
// The deadline comes from the metadata the route phase populated rather than
// from the header directly, so a Deadline interceptor's capping is what the rest
// of the call sees — reading Grpc-Timeout again here would ignore it.
func newCallCx(cx *middleware.RouteCx, started time.Time) *middleware.CallCx {
	deadline, _ := middleware.ParseGrpcTimeout(cx.Metadata.Text("grpc-timeout"))
	return &middleware.CallCx{RouteCx: cx, Started: started, Deadline: deadline}
}

// outcomeOf classifies how a call ended, for the completion phase.
//
// A nil reply with no failure is a stream that completed: its status went out
// with the first message and there is no Reply to read it from. Defaulting to
// 200 is not a guess — it is the status that was committed, and the honesty of
// a failed stream comes from the truncation, not from the status line.
func outcomeOf(reply *Reply, err *apierr.Error) middleware.Outcome {
	switch {
	case err != nil:
		return middleware.Outcome{Status: err.HTTP, Err: err}
	case reply != nil:
		return middleware.Outcome{Status: reply.Status}
	}
	return middleware.Outcome{Status: http.StatusOK}
}
