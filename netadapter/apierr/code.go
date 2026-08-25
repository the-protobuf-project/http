package apierr

// code.go holds the canonical status codes and their HTTP projection.

// Code is a google.rpc.Code.
//
// Kept as its own type rather than reusing a gRPC library's because the mapping
// to HTTP and the AIP-193 spelling are properties of this protocol, not of the
// transport underneath.
type Code int32

const (
	// OK is success. Present for completeness; an [Error] never carries it.
	OK Code = 0

	// Cancelled means the caller cancelled, usually by disconnecting. Not worth
	// reporting — there is nobody left to report it to.
	Cancelled Code = 1

	// Unknown is an error whose cause could not be determined, including a code
	// from a newer peer that this build does not recognise.
	Unknown Code = 2

	// InvalidArgument means the request is malformed regardless of system
	// state. Retrying it unchanged will fail identically.
	InvalidArgument Code = 3

	// DeadlineExceeded means the deadline elapsed before the operation
	// completed. The operation may or may not have taken effect.
	DeadlineExceeded Code = 4

	// NotFound means the named resource does not exist. Per AIP-193 it is also
	// the right answer when the caller may not know whether it exists.
	NotFound Code = 5

	// AlreadyExists means creating the resource would collide with one that
	// already exists.
	AlreadyExists Code = 6

	// PermissionDenied means the caller is authenticated but not authorized.
	// Distinct from [Unauthenticated], which means no valid credentials were
	// presented at all.
	PermissionDenied Code = 7

	// ResourceExhausted means a quota or rate limit is spent. Usually carries a
	// QuotaFailure and a RetryInfo.
	ResourceExhausted Code = 8

	// FailedPrecondition means the system is in a state the operation cannot
	// proceed from. The caller must change something before retrying.
	FailedPrecondition Code = 9

	// Aborted is a concurrency conflict, such as a failed read-modify-write.
	// Retrying the whole sequence may succeed.
	Aborted Code = 10

	// OutOfRange means a value was outside its valid range. Unlike
	// [InvalidArgument], it can become valid as system state changes.
	OutOfRange Code = 11

	// Unimplemented means the method exists in the schema but is not
	// implemented by this service.
	Unimplemented Code = 12

	// Internal means an invariant of the service itself was broken. Always a
	// bug.
	Internal Code = 13

	// Unavailable means the service is temporarily unable to answer. Retryable
	// with backoff.
	Unavailable Code = 14

	// DataLoss means data was lost or irrecoverably corrupted.
	DataLoss Code = 15

	// Unauthenticated means no valid credentials were presented. Carries a
	// WWW-Authenticate challenge describing how to authenticate.
	Unauthenticated Code = 16
)

// names is the canonical spelling AIP-193 puts in the envelope's status field.
var names = map[Code]string{
	OK: "OK", Cancelled: "CANCELLED", Unknown: "UNKNOWN",
	InvalidArgument: "INVALID_ARGUMENT", DeadlineExceeded: "DEADLINE_EXCEEDED",
	NotFound: "NOT_FOUND", AlreadyExists: "ALREADY_EXISTS",
	PermissionDenied: "PERMISSION_DENIED", ResourceExhausted: "RESOURCE_EXHAUSTED",
	FailedPrecondition: "FAILED_PRECONDITION", Aborted: "ABORTED",
	OutOfRange: "OUT_OF_RANGE", Unimplemented: "UNIMPLEMENTED", Internal: "INTERNAL",
	Unavailable: "UNAVAILABLE", DataLoss: "DATA_LOSS", Unauthenticated: "UNAUTHENTICATED",
}

// statuses is the HTTP projection from README §5.2.
//
// FailedPrecondition deliberately maps to 400 rather than the similarly named
// 412 Precondition Failed: AIP-193 says 400, and the HTTP status means
// something narrower — a failed conditional request. The one case where they
// coincide, an If-Match mismatch on an AIP-154 etag, is promoted explicitly by
// the caller.
var statuses = map[Code]int{
	OK: 200, Cancelled: 499, Unknown: 500, InvalidArgument: 400,
	DeadlineExceeded: 504, NotFound: 404, AlreadyExists: 409,
	PermissionDenied: 403, ResourceExhausted: 429, FailedPrecondition: 400,
	Aborted: 409, OutOfRange: 400, Unimplemented: 501, Internal: 500,
	Unavailable: 503, DataLoss: 500, Unauthenticated: 401,
}

// String returns the canonical enum name, which AIP-193 puts in the envelope's
// status field.
func (c Code) String() string {
	if name, ok := names[c]; ok {
		return name
	}
	return names[Unknown]
}

// HTTPStatus returns the HTTP status this code maps to.
func (c Code) HTTPStatus() int {
	if status, ok := statuses[c]; ok {
		return status
	}
	return 500
}

// Retryable reports whether a failure with this code may succeed if retried,
// which is what decides whether a RetryInfo detail is meaningful.
func (c Code) Retryable() bool {
	switch c {
	case Unavailable, ResourceExhausted, Aborted, DeadlineExceeded:
		return true
	}
	return false
}

// FromNumber builds a Code from its numeric value, mapping anything
// unrecognised to [Unknown] — which is what a code from a newer peer should
// degrade to.
func FromNumber(v int32) Code {
	if _, ok := names[Code(v)]; ok {
		return Code(v)
	}
	return Unknown
}
