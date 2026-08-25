package apierr

// invalid.go holds the failures the adapter raises about the request's content:
// a path it could not decode, a body it could not read, fields it will not
// accept.

import (
	"fmt"
	"net/http"
)

// InvalidFields reports one or more invalid request fields.
//
// Every violation is reported at once, so a caller fixes everything in one round
// trip rather than discovering problems one at a time.
func InvalidFields(violations []FieldViolation, reason, domain, method string) *Error {
	message := fmt.Sprintf("Request contains %d invalid fields.", len(violations))
	if len(violations) == 1 {
		message = fmt.Sprintf("Request contains an invalid field: %s.", violations[0].Field)
	}

	return New(InvalidArgument, message).
		WithErrorInfo(reason, domain, map[string]string{"method": method}).
		WithDetail(BadRequest{FieldViolations: violations})
}

// MalformedPath reports a captured segment that could not be percent-decoded.
//
// A 400 rather than a 404: the path matched a route, and the value is what is
// wrong. Reporting it as "not found" would send a caller looking for a resource
// when the fix is to fix their encoding.
func MalformedPath(field, description, domain, method string) *Error {
	return InvalidFields(
		[]FieldViolation{{Field: field, Description: description, Reason: "MALFORMED_PATH"}},
		"MALFORMED_PATH", domain, method,
	)
}

// MalformedBody reports a request body the codec could not decode.
func MalformedBody(description, domain, method string) *Error {
	return New(InvalidArgument, "Request body could not be decoded.").
		WithErrorInfo("MALFORMED_BODY", domain, map[string]string{
			"method": method,
			"detail": description,
		})
}

// UnexpectedBody reports a body sent to a binding that declares none.
func UnexpectedBody(domain, method string) *Error {
	return New(InvalidArgument, "This method does not accept a request body.").
		WithErrorInfo("MALFORMED_BODY", domain, map[string]string{"method": method})
}

// UnknownQueryParameter reports a query parameter no field is bound to.
//
// This is the opposite of grpc-gateway, which discards them — turning a typo in
// an update call into a silent no-op. The parameter is named as the caller
// spelled it, because that is what they have to correct.
func UnknownQueryParameter(names []string, domain, method string) *Error {
	violations := make([]FieldViolation, 0, len(names))
	for _, name := range names {
		violations = append(violations, FieldViolation{
			Field:       name,
			Description: fmt.Sprintf("Unknown query parameter %q.", name),
			Reason:      "UNKNOWN_QUERY_PARAMETER",
		})
	}
	return InvalidFields(violations, "UNKNOWN_QUERY_PARAMETER", domain, method)
}

// ReservedQueryParameter reports a "$"-prefixed parameter that is not one of
// the system parameters.
//
// The "$" prefix is reserved, so accepting an unknown one would let a future
// system parameter silently change what an existing request means.
func ReservedQueryParameter(name, domain, method string) *Error {
	return New(InvalidArgument, fmt.Sprintf("Query parameter %q is reserved.", name)).
		WithErrorInfo("UNKNOWN_QUERY_PARAMETER", domain, map[string]string{
			"method":    method,
			"parameter": name,
		})
}

// BindingMismatch reports a route table and a handler that disagree about which
// fields a route binds.
//
// A 500 rather than a 400: nothing the caller did produced it, and the fix is to
// regenerate. It exists so that failure is loud rather than a nil field three
// layers down.
func BindingMismatch(field, domain, method string) *Error {
	return New(Internal, fmt.Sprintf("Route did not bind %q.", field)).
		WithHTTP(http.StatusInternalServerError).
		WithErrorInfo("BINDING_MISMATCH", domain, map[string]string{"method": method})
}
