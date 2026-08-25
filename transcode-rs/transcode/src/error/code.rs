//! Canonical status codes and their HTTP projection.

use http::StatusCode;

/// A `google.rpc.Code`.
///
/// Kept as its own type rather than reusing `tonic::Code` because the mapping to
/// HTTP and the AIP-193 spelling are properties of this protocol, not of the
/// gRPC library underneath.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum Code {
    /// Success. Present for completeness; a `Error` never carries it.
    Ok = 0,

    /// The caller cancelled the request, usually by disconnecting. Not an error
    /// worth reporting — there is nobody left to report it to.
    Cancelled = 1,

    /// An error whose cause could not be determined, including a code from a
    /// newer peer that this build does not recognise.
    Unknown = 2,

    /// The request is malformed regardless of system state: a bad field, an
    /// unparseable value, an unknown query parameter. Retrying without changing
    /// the request will fail identically.
    InvalidArgument = 3,

    /// The deadline elapsed before the operation completed. The operation may
    /// or may not have taken effect.
    DeadlineExceeded = 4,

    /// The named resource does not exist. Per AIP-193 this is also the correct
    /// answer when the caller is not permitted to know whether it exists.
    NotFound = 5,

    /// Creating the resource would collide with one that already exists.
    AlreadyExists = 6,

    /// The caller is authenticated but not authorized. Distinct from
    /// [`Code::Unauthenticated`], which means no valid credentials were
    /// presented at all.
    PermissionDenied = 7,

    /// A quota or rate limit is exhausted. Usually accompanied by a
    /// `QuotaFailure` and a `RetryInfo`.
    ResourceExhausted = 8,

    /// The system is in a state the operation cannot proceed from — deleting a
    /// non-empty directory, acting on an unverified account. The caller must
    /// change something before retrying.
    FailedPrecondition = 9,

    /// A concurrency conflict, such as a failed read-modify-write. Retrying the
    /// whole sequence may succeed.
    Aborted = 10,

    /// A value was outside its valid range — a page token past the end, a
    /// negative page size. Unlike [`Code::InvalidArgument`], this can become
    /// valid as system state changes.
    OutOfRange = 11,

    /// The method exists in the schema but is not implemented by this service.
    Unimplemented = 12,

    /// An invariant of the service itself was broken. Always a bug.
    Internal = 13,

    /// The service is temporarily unable to answer. Retryable with backoff.
    Unavailable = 14,

    /// Data was lost or irrecoverably corrupted.
    DataLoss = 15,

    /// No valid credentials were presented. Carries a `WWW-Authenticate`
    /// challenge describing how to authenticate.
    Unauthenticated = 16,
}

impl Code {
    /// The canonical enum name, which AIP-193 puts in the envelope's `status`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Code::Ok => "OK",
            Code::Cancelled => "CANCELLED",
            Code::Unknown => "UNKNOWN",
            Code::InvalidArgument => "INVALID_ARGUMENT",
            Code::DeadlineExceeded => "DEADLINE_EXCEEDED",
            Code::NotFound => "NOT_FOUND",
            Code::AlreadyExists => "ALREADY_EXISTS",
            Code::PermissionDenied => "PERMISSION_DENIED",
            Code::ResourceExhausted => "RESOURCE_EXHAUSTED",
            Code::FailedPrecondition => "FAILED_PRECONDITION",
            Code::Aborted => "ABORTED",
            Code::OutOfRange => "OUT_OF_RANGE",
            Code::Unimplemented => "UNIMPLEMENTED",
            Code::Internal => "INTERNAL",
            Code::Unavailable => "UNAVAILABLE",
            Code::DataLoss => "DATA_LOSS",
            Code::Unauthenticated => "UNAUTHENTICATED",
        }
    }

    /// The HTTP status this code maps to, per README §5.2
    ///
    /// `FAILED_PRECONDITION` deliberately maps to `400` rather than the
    /// similarly named `412 Precondition Failed`: AIP-193 says `400`, and the
    /// HTTP status means something narrower — a failed conditional request.
    /// The one case where they coincide, an `If-Match` mismatch on an AIP-154
    /// etag, is promoted explicitly by the caller.
    pub const fn http_status(self) -> u16 {
        match self {
            Code::Ok => 200,
            Code::Cancelled => 499, // "Client Closed Request", nginx's code
            Code::Unknown => 500,
            Code::InvalidArgument => 400,
            Code::DeadlineExceeded => 504,
            Code::NotFound => 404,
            Code::AlreadyExists => 409,
            Code::PermissionDenied => 403,
            Code::ResourceExhausted => 429,
            Code::FailedPrecondition => 400,
            Code::Aborted => 409,
            Code::OutOfRange => 400,
            Code::Unimplemented => 501,
            Code::Internal => 500,
            Code::Unavailable => 503,
            Code::DataLoss => 500,
            Code::Unauthenticated => 401,
        }
    }

    /// The HTTP status as an [`StatusCode`].
    ///
    /// Total by construction: every value [`Code::http_status`] returns is a
    /// literal in the range `StatusCode` accepts, so the fallback is
    /// unreachable rather than a panic waiting to be hit.
    pub fn status_code(self) -> StatusCode {
        StatusCode::from_u16(self.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
    }

    /// Whether a failure with this code may succeed if retried, which drives
    /// whether a `RetryInfo` detail is meaningful.
    pub const fn retryable(self) -> bool {
        matches!(
            self,
            Code::Unavailable | Code::ResourceExhausted | Code::Aborted | Code::DeadlineExceeded
        )
    }

    /// Builds a `Code` from its numeric value, mapping anything unrecognised to
    /// `Unknown` — which is what a code from a newer peer should degrade to.
    pub const fn from_i32(v: i32) -> Code {
        match v {
            0 => Code::Ok,
            1 => Code::Cancelled,
            3 => Code::InvalidArgument,
            4 => Code::DeadlineExceeded,
            5 => Code::NotFound,
            6 => Code::AlreadyExists,
            7 => Code::PermissionDenied,
            8 => Code::ResourceExhausted,
            9 => Code::FailedPrecondition,
            10 => Code::Aborted,
            11 => Code::OutOfRange,
            12 => Code::Unimplemented,
            13 => Code::Internal,
            14 => Code::Unavailable,
            15 => Code::DataLoss,
            16 => Code::Unauthenticated,
            _ => Code::Unknown,
        }
    }
}

impl From<tonic::Code> for Code {
    fn from(c: tonic::Code) -> Self {
        Code::from_i32(c as i32)
    }
}
