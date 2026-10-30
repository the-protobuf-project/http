//! The resolved request a dispatcher receives.

use super::query::Query;
use super::reply::Reply;
use crate::codec::{self, CodecEntry, Decode, Encode, Payload};
use crate::error::{Code, Error, FieldViolation, Result};
use crate::middleware::{CallCx, Extensions, Headers, Metadata, MetadataValue};
use crate::route::{MethodInfo, Route};
use bytes::{Bytes, BytesMut};
use http::{HeaderMap, HeaderValue, StatusCode, header};
use std::collections::HashMap;
use tonic::metadata::{AsciiMetadataKey, BinaryMetadataKey, MetadataMap};

/// A resolved request, ready for a method handler.
///
/// Captures are already decoded; the body is still bytes, because which codec
/// decodes it was decided by negotiation and only the handler knows its types.
#[derive(Debug)]
pub struct Call<'a> {
    /// The middleware context, kept private because a handler reaches it
    /// through [`Call::context`]. Exposing it mutably would invite a handler to
    /// change what the route phase already decided.
    cx: &'a CallCx<'a>,
    /// Maps metadata names back to response headers.
    headers: &'a Headers,
    /// The codec a body decodes with when the request named none.
    default_codec: &'static CodecEntry,

    /// The method being served, from the generated method table.
    pub method: &'static MethodInfo,
    /// The binding that matched, for diagnostics and tracing.
    pub route: &'static Route,
    /// The query parameters, with the system parameters removed.
    pub query: &'a Query,
    /// The raw request body, empty when none was sent.
    pub body: Bytes,
    /// The codec the body decodes with, `None` when the request had no
    /// `Content-Type`.
    pub request_codec: Option<&'static CodecEntry>,
    /// The negotiated response codec.
    pub response_codec: &'static CodecEntry,
    /// The API's error domain, so a handler raising an error does not have to
    /// thread it separately.
    pub domain: &'static str,
}

impl<'a> Call<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        cx: &'a CallCx<'a>,
        headers: &'a Headers,
        default_codec: &'static CodecEntry,
        method: &'static MethodInfo,
        route: &'static Route,
        query: &'a Query,
        body: Bytes,
        request_codec: Option<&'static CodecEntry>,
        response_codec: &'static CodecEntry,
        domain: &'static str,
    ) -> Self {
        Self {
            cx,
            headers,
            default_codec,
            method,
            route,
            query,
            body,
            request_codec,
            response_codec,
            domain,
        }
    }

    /// The method's index in the method table, which is what a generated
    /// dispatch switches on.
    #[must_use]
    pub const fn handler(&self) -> usize {
        self.route.handler
    }

    /// The call's context, carrying whatever the interceptors decided — the
    /// deadline, the resolved peer, the forwarded metadata.
    #[must_use]
    pub const fn context(&self) -> &CallCx<'a> {
        self.cx
    }

    /// What will be forwarded to the service, as the incoming matcher and any
    /// annotators built it.
    #[must_use]
    pub const fn metadata(&self) -> &Metadata {
        &self.cx.route.metadata
    }

    /// Values the interceptors left for the handler — an authenticated
    /// identity, say.
    #[must_use]
    pub const fn extensions(&self) -> &Extensions {
        &self.cx.route.extensions
    }

    /// The request headers, as received.
    #[must_use]
    pub const fn http_headers(&self) -> &HeaderMap {
        self.cx.route.headers
    }

    /// Every path capture, keyed by protojson field path.
    #[must_use]
    pub fn captures(&self) -> &HashMap<&'static str, String> {
        self.cx.route.captures
    }

    /// A required path capture.
    ///
    /// # Errors
    ///
    /// A missing capture means the route table and the handler disagree, which
    /// is a generator bug rather than a caller error — so it is a 500.
    pub fn capture(&self, field: &str) -> Result<&str> {
        self.cx
            .route
            .captures
            .get(field)
            .map(String::as_str)
            .ok_or_else(|| {
                Box::new(Error::binding_mismatch(
                    field,
                    self.domain,
                    self.method.full_name,
                ))
            })
    }

    /// Rejects every query parameter not in `known`.
    ///
    /// The bindable set is a property of the binding, which the generated
    /// handler knows and the runtime does not, so the check is offered rather
    /// than performed automatically.
    ///
    /// # Errors
    ///
    /// A 400 naming every unknown parameter.
    pub fn reject_unknown_query(&self, known: &[&str]) -> Result<()> {
        let unknown = self.query.unknown(known);
        if unknown.is_empty() {
            return Ok(());
        }
        Err(Box::new(Error::unknown_query_parameter(
            &unknown,
            self.domain,
            self.method.full_name,
        )))
    }

    /// A 400 naming the fields that failed validation.
    #[must_use]
    pub fn invalid(&self, violations: Vec<FieldViolation>) -> Box<Error> {
        Box::new(Error::invalid_fields(
            violations,
            "INVALID_ARGUMENT",
            self.domain,
            self.method.full_name,
        ))
    }

    /// An error with the given code, carrying this call's domain and method in
    /// its `ErrorInfo`.
    #[must_use]
    pub fn error(&self, code: Code, message: impl Into<String>) -> Box<Error> {
        Box::new(Error::new(code, message).with_error_info(
            code.as_str(),
            self.domain,
            [("method".into(), self.method.full_name.into())],
        ))
    }

    /// Maps a service's status into this call's error.
    ///
    /// # Errors
    ///
    /// The status, rendered with its details and this call's domain.
    pub fn rpc<T>(&self, result: std::result::Result<T, tonic::Status>) -> Result<T> {
        result.map_err(|status| Box::new(Error::from_status(&status, self.domain)))
    }

    /// Decodes the body with the negotiated request codec.
    ///
    /// # Errors
    ///
    /// A 400 naming what could not be decoded.
    pub fn decode<M: Payload>(&self) -> Result<M> {
        let entry = self.request_codec.unwrap_or(self.default_codec);
        codec::payload::decode(entry, &self.body)
            .map_err(|err| Box::new(err.into_gateway_error(self.domain, self.method.full_name)))
    }

    /// Encodes a message with the negotiated response codec.
    ///
    /// # Errors
    ///
    /// A 500 when the codec fails.
    pub fn encode<M: Payload>(&self, message: &M) -> Result<Bytes> {
        let mut buf = BytesMut::new();
        codec::payload::encode(self.response_codec, message, &mut buf)
            .map_err(|err| Box::new(err.into_gateway_error(self.domain, self.method.full_name)))?;
        Ok(buf.freeze())
    }

    /// A reply with the given status, the message encoded with the negotiated
    /// codec.
    ///
    /// # Errors
    ///
    /// A 500 when the codec fails.
    pub fn reply<M: Payload>(&self, status: StatusCode, message: &M) -> Result<Reply> {
        let mut reply = Reply::new(status, self.encode(message)?);
        if let Ok(value) = HeaderValue::from_str(self.response_codec.content_type()) {
            reply.headers.insert(header::CONTENT_TYPE, value);
        }
        Ok(reply)
    }

    /// Decodes the body with a codec the caller names rather than the one
    /// negotiation chose.
    ///
    /// For a message type that does not satisfy [`Payload`] — a hand-written
    /// serde type in a build that also enables the binary codec, say. Generated
    /// prost messages use [`Call::decode`].
    ///
    /// # Errors
    ///
    /// A 400 naming what could not be decoded.
    pub fn decode_with<M, C: Decode<M>>(&self, codec: &C) -> Result<M> {
        codec
            .decode(&self.body)
            .map_err(|err| Box::new(err.into_gateway_error(self.domain, self.method.full_name)))
    }

    /// A reply encoded with a codec the caller names, labelled with that
    /// codec's content type. The counterpart of [`Call::decode_with`].
    ///
    /// # Errors
    ///
    /// A 500 when the codec fails.
    pub fn reply_with<M, C: Encode<M>>(
        &self,
        codec: &C,
        status: StatusCode,
        message: &M,
    ) -> Result<Reply> {
        let mut buf = BytesMut::new();
        codec
            .encode(message, &mut buf)
            .map_err(|err| Box::new(err.into_gateway_error(self.domain, self.method.full_name)))?;
        Ok(
            Reply::new(status, buf.freeze())
                .with_header(header::CONTENT_TYPE, codec.content_type()),
        )
    }

    /// A `200` carrying the message.
    ///
    /// # Errors
    ///
    /// A 500 when the codec fails.
    pub fn ok<M: Payload>(&self, message: &M) -> Result<Reply> {
        self.reply(StatusCode::OK, message)
    }

    /// An AIP-133 `201 Created` carrying the message and its `Location`.
    ///
    /// # Errors
    ///
    /// A 500 when the codec fails.
    pub fn created<M: Payload>(&self, message: &M, location: &str) -> Result<Reply> {
        Ok(self
            .reply(StatusCode::CREATED, message)?
            .with_header(header::LOCATION, location))
    }

    /// A `204 No Content`, for an AIP-135 Delete returning `Empty`.
    #[must_use]
    pub fn no_content(&self) -> Reply {
        Reply::no_content()
    }

    /// Builds the request a gRPC service is called with: the message, the
    /// forwarded metadata, and what remains of the deadline.
    ///
    /// This is what lets a dispatcher call a tonic client — or a service
    /// implementation directly — and have it see exactly what a gRPC caller's
    /// request would have carried.
    pub fn request<M>(&self, message: M) -> tonic::Request<M> {
        let mut request = tonic::Request::new(message);
        copy_metadata(self.metadata(), request.metadata_mut());
        if let Some(deadline) = self.cx.deadline {
            request.set_timeout(deadline.saturating_sub(self.cx.elapsed()));
        }
        request
    }

    /// Renders a service's response: the message with the negotiated codec, and
    /// its metadata projected into response headers by the outgoing matcher.
    ///
    /// # Errors
    ///
    /// The service's status, or a 500 when the codec fails.
    pub fn respond<M: Payload>(
        &self,
        result: std::result::Result<tonic::Response<M>, tonic::Status>,
    ) -> Result<Reply> {
        let response = self.rpc(result)?;
        let (metadata, message, _) = response.into_parts();
        let mut reply = self.ok(&message)?;
        project_metadata(&metadata, self.headers, &mut reply.headers);
        Ok(reply)
    }
}

/// Copies forwarded metadata into a tonic request, dropping anything the gRPC
/// metadata grammar rejects rather than failing the call over it.
fn copy_metadata(from: &Metadata, to: &mut MetadataMap) {
    for key in from.keys() {
        for value in from.get(key).unwrap_or_default() {
            match value {
                MetadataValue::Text(text) => {
                    if let (Ok(key), Ok(value)) = (key.parse::<AsciiMetadataKey>(), text.parse()) {
                        to.append(key, value);
                    }
                }
                MetadataValue::Binary(bytes) => {
                    if let Ok(key) = BinaryMetadataKey::from_bytes(key.as_bytes()) {
                        to.append_bin(key, tonic::metadata::MetadataValue::from_bytes(bytes));
                    }
                }
            }
        }
    }
}

/// Projects a response's text metadata into headers through the outgoing
/// matcher, which is grpc-gateway's `Grpc-Metadata-` forwarding.
fn project_metadata(from: &MetadataMap, headers: &Headers, to: &mut HeaderMap) {
    for entry in from.iter() {
        if let tonic::metadata::KeyAndValueRef::Ascii(key, value) = entry
            && let Some(name) = headers.outgoing.translate(key.as_str())
            && let (Ok(name), Ok(value)) = (
                header::HeaderName::from_bytes(name.as_bytes()),
                HeaderValue::from_bytes(value.as_encoded_bytes()),
            )
        {
            to.append(name, value);
        }
    }
}
