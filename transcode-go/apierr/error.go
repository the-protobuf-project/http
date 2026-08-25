package apierr

// error.go is the error type every failure funnels through.

import (
	"encoding/json"
	"net/http"
)

// Error is a failure, ready to be rendered as an HTTP response.
type Error struct {
	// Code is the canonical code, which becomes the envelope's status.
	Code Code

	// HTTP is the HTTP status.
	//
	// Normally Code.HTTPStatus(), but a caller may promote it — an If-Match
	// mismatch turning FAILED_PRECONDITION into 412, or a routing failure
	// keeping its 405 instead of following UNIMPLEMENTED to 501.
	HTTP int

	// Message is the human-readable message. Superseded by a
	// [LocalizedMessage] detail when the service supplies one.
	Message string

	// Details are the google.rpc details rendered into the envelope.
	Details []Detail

	// Header carries headers the failure sets directly, such as Allow on a 405.
	// Headers derived from details are added when the error is written.
	Header http.Header
}

// New builds an error with the canonical HTTP status for code.
func New(code Code, message string) *Error {
	return &Error{
		Code:    code,
		HTTP:    code.HTTPStatus(),
		Message: message,
		Header:  http.Header{},
	}
}

// Error implements the error interface.
func (e *Error) Error() string { return e.Code.String() + ": " + e.Message }

// WithHTTP overrides the HTTP status, for the narrow cases where the canonical
// mapping is not the most accurate answer.
func (e *Error) WithHTTP(status int) *Error {
	e.HTTP = status
	return e
}

// WithDetail appends one detail.
func (e *Error) WithDetail(detail Detail) *Error {
	e.Details = append(e.Details, detail)
	return e
}

// WithErrorInfo attaches the [ErrorInfo] AIP-193 requires on every error.
func (e *Error) WithErrorInfo(reason, domain string, metadata map[string]string) *Error {
	return e.WithDetail(ErrorInfo{Reason: reason, Domain: domain, Metadata: metadata})
}

// WithHeader sets one response header.
func (e *Error) WithHeader(name, value string) *Error {
	if e.Header == nil {
		e.Header = http.Header{}
	}
	e.Header.Set(name, value)
	return e
}

// EnsureErrorInfo guarantees an [ErrorInfo] is present, synthesising one from
// the code when the service returned none.
//
// It goes first in the details array, because it is the detail a caller reads
// to decide what to do and the array has no other ordering.
func (e *Error) EnsureErrorInfo(domain string) *Error {
	if e.ErrorInfo() != nil {
		return e
	}
	synthesised := ErrorInfo{Reason: e.Code.String(), Domain: domain}
	e.Details = append([]Detail{synthesised}, e.Details...)
	return e
}

// ErrorInfo returns the attached [ErrorInfo], or nil.
func (e *Error) ErrorInfo() *ErrorInfo {
	for _, detail := range e.Details {
		if info, ok := detail.(ErrorInfo); ok {
			return &info
		}
	}
	return nil
}

// StripDebugInfo removes [DebugInfo] details, which can describe the shape of
// the service.
func (e *Error) StripDebugInfo() *Error {
	kept := e.Details[:0]
	for _, detail := range e.Details {
		if _, isDebug := detail.(DebugInfo); !isDebug {
			kept = append(kept, detail)
		}
	}
	e.Details = kept
	return e
}

// RenderedMessage is the message to render: a [LocalizedMessage] when the
// service supplied one, otherwise [Error.Message].
func (e *Error) RenderedMessage() string {
	for _, detail := range e.Details {
		if localized, ok := detail.(LocalizedMessage); ok && localized.Message != "" {
			return localized.Message
		}
	}
	return e.Message
}

// envelope is the AIP-193 body shape. The field order is the order they are
// declared in, which is the order the specification writes them.
type envelope struct {
	Error envelopeBody `json:"error"`
}

// envelopeBody is the object under the envelope's single "error" key.
type envelopeBody struct {
	// Code is the HTTP status, not the gRPC code. That single difference is why
	// a grpc-gateway error body reports 3 for a bad request: it serializes the
	// raw google.rpc.Status, whose code field holds the canonical code's
	// number, which is not an HTTP status at all.
	Code int `json:"code"`

	// Message is the human-readable message.
	Message string `json:"message"`

	// Status is the canonical google.rpc.Code name.
	Status string `json:"status"`

	// Details are the rendered details, omitted when there are none.
	Details []map[string]any `json:"details,omitempty"`
}

// JSON renders the AIP-193 envelope.
func (e *Error) JSON() ([]byte, error) {
	details := make([]map[string]any, 0, len(e.Details))
	for _, detail := range e.Details {
		rendered := map[string]any{"@type": detail.TypeURL()}
		for key, value := range detail.Fields() {
			rendered[key] = value
		}
		details = append(details, rendered)
	}
	if len(details) == 0 {
		details = nil
	}

	return json.Marshal(envelope{Error: envelopeBody{
		Code:    e.HTTP,
		Message: e.RenderedMessage(),
		Status:  e.Code.String(),
		Details: details,
	}})
}
