package netadapter

// render.go turns a failure into a response, and catches the ones nothing else
// did.

import (
	"errors"
	"net/http"

	"github.com/the-protobuf-project/grpc-gateway-rs/netadapter/apierr"
	"github.com/the-protobuf-project/grpc-gateway-rs/netadapter/route"
)

// renderError writes any failure as an AIP-193 response.
func (a *Adapter) renderError(w http.ResponseWriter, err error) {
	errorReply(a.asGatewayError(err)).Write(w)
}

// asGatewayError normalizes any error into a [*apierr.Error].
//
// An error the adapter cannot classify becomes an INTERNAL with a generic
// message: its text may name a table, a host, or a query, and a caller who
// cannot act on the detail should not receive it. The original is logged
// instead, where an operator can.
func (a *Adapter) asGatewayError(err error) *apierr.Error {
	var failure *apierr.Error
	if !errors.As(err, &failure) {
		a.options.Logger.Error("unclassified handler error", "error", err)
		failure = apierr.New(apierr.Internal, "Internal error.")
	}

	failure = failure.EnsureErrorInfo(a.domain)
	if !a.options.ExposeDebugInfo {
		failure = failure.StripDebugInfo()
	}
	return failure
}

// malformedPath renders a capture that could not be percent-decoded.
//
// A 400 naming the field rather than a 404: the path matched a route, and the
// value is what is wrong. Telling a caller "not found" would send them looking
// for a resource when the fix is to fix their encoding.
func malformedPath(err error, domain, method string) *apierr.Error {
	var captureErr *route.CaptureError
	if errors.As(err, &captureErr) {
		return apierr.MalformedPath(captureErr.Field, captureErr.Err.Description(), domain, method)
	}
	return apierr.MalformedPath("", err.Error(), domain, method)
}

// recoverPanic renders a panicked handler as an ordinary 500.
//
// http.ErrAbortHandler is re-panicked rather than caught: it is how a handler
// asks for the response to be terminated abnormally, which is exactly what a
// stream that failed after committing needs. Catching it here would convert
// that deliberate truncation into a clean close, which is the lie this whole
// design exists to prevent.
func (a *Adapter) recoverPanic(w http.ResponseWriter) {
	recovered := recover()
	if recovered == nil {
		return
	}
	if recovered == http.ErrAbortHandler {
		panic(recovered)
	}

	a.options.Logger.Error("handler panicked", "panic", recovered)
	// The payload never reaches the client, and the connection is not dropped:
	// an unwind in one handler is not a reason to fail every in-flight request
	// sharing that connection.
	errorReply(apierr.Panicked(a.domain, "").EnsureErrorInfo(a.domain)).Write(w)
}
