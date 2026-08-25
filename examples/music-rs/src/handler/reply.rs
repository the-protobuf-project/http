//! Building an encoded response.

use super::{Call, Reply};
use crate::generated::DOMAIN;
use bytes::BytesMut;
use grpc_http::codec::{Encode, JsonCodec};
use grpc_http::error::GatewayError;
use http::{HeaderMap, HeaderValue, StatusCode};
use serde::Serialize;

impl Call<'_> {
    /// Encodes a `200` response.
    ///
    /// # Errors
    ///
    /// `500` when the message cannot be encoded, which is a service bug rather
    /// than anything the caller did.
    pub fn ok<M: Serialize>(&self, message: &M) -> Result<Reply, Box<GatewayError>> {
        self.reply(message, StatusCode::OK, None)
    }

    /// Encodes a `201 Created` with a `Location` header. (AIP-133)
    ///
    /// # Errors
    ///
    /// As [`Call::ok`].
    pub fn created<M: Serialize>(
        &self,
        message: &M,
        location: &str,
    ) -> Result<Reply, Box<GatewayError>> {
        self.reply(message, StatusCode::CREATED, Some(location))
    }

    /// Encodes a response with an optional `Location`.
    fn reply<M: Serialize>(
        &self,
        message: &M,
        status: StatusCode,
        location: Option<&str>,
    ) -> Result<Reply, Box<GatewayError>> {
        let mut buf = BytesMut::new();
        JsonCodec::new()
            .encode(message, &mut buf)
            .map_err(|err| Box::new(err.into_gateway_error(DOMAIN, self.method.full_name())))?;

        let mut headers = HeaderMap::new();
        headers.insert(
            http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        if let Some(location) = location
            && let Ok(value) = HeaderValue::from_str(location)
        {
            headers.insert(http::header::LOCATION, value);
        }
        Ok(Reply {
            status,
            headers,
            body: buf.to_vec(),
        })
    }
}
