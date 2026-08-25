//! Converting a `tonic::Status` into the handler's error model.

use super::details::Detail;
use super::{Code, Error};
use prost::Message;

/// `google.rpc.Status`, the wire form of a gRPC error's details trailer.
///
/// Declared locally for the same reason the detail types are: rendering an
/// error must not depend on generated code.
#[derive(Clone, PartialEq, Message)]
struct RpcStatus {
    /// The canonical code's number.
    #[prost(int32, tag = "1")]
    code: i32,
    /// The developer-facing message.
    #[prost(string, tag = "2")]
    message: String,
    /// The attached details, each a `google.protobuf.Any`.
    #[prost(message, repeated, tag = "3")]
    details: Vec<prost_types::Any>,
}

impl Error {
    /// Converts a `tonic::Status`, decoding any `google.rpc.Status` details it
    /// carries.
    ///
    /// `domain` seeds the `ErrorInfo` when the service supplied none.
    pub fn from_status(status: &tonic::Status, domain: &str) -> Self {
        let mut err = Error::new(Code::from(status.code()), status.message());

        if !status.details().is_empty()
            && let Ok(rpc) = RpcStatus::decode(status.details())
        {
            // The trailer's message is authoritative when present: tonic's own
            // message field comes from a header, which is length-capped and may
            // therefore be a truncated copy.
            if !rpc.message.is_empty() {
                err.message = rpc.message;
            }
            err.details = rpc.details.iter().map(Detail::from_any).collect();
        }
        err.ensure_error_info(domain)
    }
}
