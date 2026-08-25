//! Building an encoded response.

use super::{Call, Reply};
use crate::generated::DOMAIN;
use bytes::BytesMut;
use http::{HeaderMap, HeaderValue, StatusCode};
use serde::Serialize;
use transcode::codec::{Encode, JsonCodec};
use transcode::error::Error;

impl Call<'_> {
    /// Encodes a `200` response.
    ///
    /// # Errors
    ///
    /// `500` when the message cannot be encoded, which is a service bug rather
    /// than anything the caller did.
    pub fn ok<M: Serialize>(&self, message: &M) -> Result<Reply, Box<Error>> {
        self.reply(message, StatusCode::OK, None)
    }

    /// Encodes a `201 Created` with a `Location` header. (AIP-133)
    ///
    /// # Errors
    ///
    /// As [`Call::ok`].
    pub fn created<M: Serialize>(&self, message: &M, location: &str) -> Result<Reply, Box<Error>> {
        self.reply(message, StatusCode::CREATED, Some(location))
    }

    /// Encodes a response with an optional `Location`.
    fn reply<M: Serialize>(
        &self,
        message: &M,
        status: StatusCode,
        location: Option<&str>,
    ) -> Result<Reply, Box<Error>> {
        let mut buf = BytesMut::new();
        JsonCodec::new()
            .encode(message, &mut buf)
            .map_err(|err| Box::new(err.into_gateway_error(DOMAIN, self.method.full_name())))?;

        let mut headers = HeaderMap::new();
        // From the negotiated codec rather than a literal: a response that
        // announces a media type the caller did not get is worse than one that
        // announces nothing, and hardcoding it here is how the two drift.
        if let Ok(value) = HeaderValue::from_str(self.codecs.response.content_type()) {
            headers.insert(http::header::CONTENT_TYPE, value);
        }
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
