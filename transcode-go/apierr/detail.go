package apierr

// detail.go declares the google.rpc detail types an AIP-193 envelope carries.
//
// They are declared here rather than reused from generated protobuf code
// because rendering an error must not depend on a generated package: the
// transcoder has to be able to report a failure that happened before any message
// type was involved.

// Detail is one entry of an error envelope's details array.
//
// The interface is deliberately narrow. A detail is a type URL and a JSON
// object, which is all the envelope needs; anything richer would put protobuf
// back on the error path.
type Detail interface {
	// TypeURL is the "@type" the detail is rendered with.
	TypeURL() string

	// Fields returns the detail's JSON body, without "@type".
	Fields() map[string]any
}

// typePrefix is the type URL every google.rpc detail is published under.
const typePrefix = "type.googleapis.com/google.rpc."

// ErrorInfo identifies the failure in machine-readable terms.
//
// AIP-193 requires exactly one on every error, because a caller who cannot tell
// which service failed, or why, cannot act on it. A service returning none gets
// one synthesised.
type ErrorInfo struct {
	// Reason is a stable, screaming-snake token, e.a. "ROUTE_NOT_FOUND".
	Reason string

	// Domain is the API's error domain, e.a. "library.example.com".
	Domain string

	// Metadata is additional key-value context, e.a. the offending path.
	Metadata map[string]string
}

// TypeURL implements [Detail].
func (ErrorInfo) TypeURL() string { return typePrefix + "ErrorInfo" }

// Fields implements [Detail].
func (e ErrorInfo) Fields() map[string]any {
	out := map[string]any{"reason": e.Reason, "domain": e.Domain}
	if len(e.Metadata) > 0 {
		out["metadata"] = e.Metadata
	}
	return out
}

// FieldViolation is one invalid request field.
type FieldViolation struct {
	// Field is the protojson path, so it names what the client sent and what
	// OpenAPI documents.
	Field string

	// Description says what is wrong, in terms the caller can act on.
	Description string

	// Reason is a stable token, e.a. "VALUE_LENGTH".
	Reason string
}

// BadRequest carries every field violation of one request.
//
// Every violation is reported at once so a caller fixes everything in one round
// trip rather than discovering problems one at a time.
type BadRequest struct {
	// FieldViolations are the invalid fields, in the order they were found.
	FieldViolations []FieldViolation
}

// TypeURL implements [Detail].
func (BadRequest) TypeURL() string { return typePrefix + "BadRequest" }

// Fields implements [Detail].
func (b BadRequest) Fields() map[string]any {
	violations := make([]any, 0, len(b.FieldViolations))
	for _, v := range b.FieldViolations {
		entry := map[string]any{"field": v.Field, "description": v.Description}
		if v.Reason != "" {
			entry["reason"] = v.Reason
		}
		violations = append(violations, entry)
	}
	return map[string]any{"fieldViolations": violations}
}

// LocalizedMessage is a message in the caller's locale. When present it
// supersedes the envelope's own message.
type LocalizedMessage struct {
	// Locale is an IETF BCP 47 tag, e.a. "en-US".
	Locale string

	// Message is the localized text.
	Message string
}

// TypeURL implements [Detail].
func (LocalizedMessage) TypeURL() string { return typePrefix + "LocalizedMessage" }

// Fields implements [Detail].
func (l LocalizedMessage) Fields() map[string]any {
	return map[string]any{"locale": l.Locale, "message": l.Message}
}

// DebugInfo is diagnostic detail that can describe the shape of the service.
//
// Stripped unless the runtime is explicitly configured to expose it, which it
// should refuse to be on a non-loopback listener.
type DebugInfo struct {
	// StackEntries are frames, innermost first.
	StackEntries []string

	// Detail is free-form diagnostic text.
	Detail string
}

// TypeURL implements [Detail].
func (DebugInfo) TypeURL() string { return typePrefix + "DebugInfo" }

// Fields implements [Detail].
func (d DebugInfo) Fields() map[string]any {
	return map[string]any{"stackEntries": d.StackEntries, "detail": d.Detail}
}
