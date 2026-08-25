package builtin

// validate.go rejects an invalid request before the RPC is dialled.

import (
	"github.com/the-protobuf-project/http/transcode-go/apierr"
	"github.com/the-protobuf-project/http/transcode-go/middleware"
)

// Validator checks a bound request message.
//
// Generated per message from the four sources in README §2.1: AIP-203 field
// behaviour, AIP-122/123 resource patterns, google.api.field_info formats, and
// protovalidate constraints. Three of the four compile to direct code; only CEL
// needs an evaluator at runtime.
type Validator[M any] interface {
	// Validate appends every violation in the message.
	//
	// Collecting rather than returning at the first is the whole point: a
	// caller with three bad fields should learn about three, not discover them
	// one round trip at a time.
	Validate(message *M, out *[]apierr.FieldViolation)
}

// Validate rejects an invalid request before the RPC is dialled.
//
// This is what grpc-gateway has no place for. Its extension points all sit
// either side of the message — WithMetadata before it exists,
// WithForwardResponseOption after the call — so there is no hook that can see a
// decoded request, and validation ends up in every service instead.
//
// Adapter-side validation is defence in depth, not a substitute for the
// service's own: a service must still assume unvalidated input, because the
// transcoder is not the only way in. What this buys is a good error at the edge and
// a truthful OpenAPI document.
type Validate[M any] struct {
	// validator collects the violations.
	validator Validator[M]

	// domain is the API's error domain.
	domain string
}

// NewValidate returns the interceptor for one message type.
func NewValidate[M any](validator Validator[M], domain string) *Validate[M] {
	return &Validate[M]{validator: validator, domain: domain}
}

// Name implements [middleware.Interceptor].
func (*Validate[M]) Name() string { return "validate" }

// InspectRequest implements [middleware.InspectRequest].
func (v *Validate[M]) InspectRequest(cx *middleware.CallCx, message *M) error {
	var violations []apierr.FieldViolation
	v.validator.Validate(message, &violations)

	if len(violations) == 0 {
		return nil
	}
	return apierr.InvalidFields(violations, "INVALID_ARGUMENT", v.domain, cx.Method.FullName)
}
