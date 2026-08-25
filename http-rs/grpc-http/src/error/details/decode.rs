//! Decoding details out of a `google.rpc.Status` trailer.

use super::Detail;
use super::types::*;
use prost::Message;

impl Detail {
    /// Decodes one `google.protobuf.Any` from a service's status details.
    ///
    /// A detail that fails to decode is kept as [`Detail::Unknown`] rather than
    /// dropped: losing it would turn a partially understood error into a
    /// silently truncated one, which is harder to diagnose than an opaque one.
    pub fn from_any(any: &prost_types::Any) -> Detail {
        let name = any
            .type_url
            .rsplit_once('/')
            .map(|(_, n)| n)
            .unwrap_or(&any.type_url);
        let buf = any.value.as_slice();

        match name {
            "google.rpc.ErrorInfo" => or_unknown(ErrorInfo::decode(buf), Detail::ErrorInfo, any),
            "google.rpc.BadRequest" => or_unknown(BadRequest::decode(buf), Detail::BadRequest, any),
            "google.rpc.RetryInfo" => or_unknown(RetryInfo::decode(buf), Detail::RetryInfo, any),
            "google.rpc.QuotaFailure" => {
                or_unknown(QuotaFailure::decode(buf), Detail::QuotaFailure, any)
            }
            "google.rpc.PreconditionFailure" => or_unknown(
                PreconditionFailure::decode(buf),
                Detail::PreconditionFailure,
                any,
            ),
            "google.rpc.ResourceInfo" => {
                or_unknown(ResourceInfo::decode(buf), Detail::ResourceInfo, any)
            }
            "google.rpc.Help" => or_unknown(Help::decode(buf), Detail::Help, any),
            "google.rpc.LocalizedMessage" => {
                or_unknown(LocalizedMessage::decode(buf), Detail::LocalizedMessage, any)
            }
            "google.rpc.DebugInfo" => or_unknown(DebugInfo::decode(buf), Detail::DebugInfo, any),
            "google.rpc.RequestInfo" => {
                or_unknown(RequestInfo::decode(buf), Detail::RequestInfo, any)
            }
            _ => unknown(any),
        }
    }
}

/// Wraps a successful decode, or falls back to preserving the raw payload.
fn or_unknown<T, F>(
    decoded: Result<T, prost::DecodeError>,
    wrap: F,
    any: &prost_types::Any,
) -> Detail
where
    F: FnOnce(T) -> Detail,
{
    decoded.map(wrap).unwrap_or_else(|_| unknown(any))
}

/// Preserves a detail this crate cannot interpret.
fn unknown(any: &prost_types::Any) -> Detail {
    Detail::Unknown {
        type_url: any.type_url.clone(),
        value: any.value.clone(),
    }
}
