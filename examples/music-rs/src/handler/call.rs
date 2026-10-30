//! What the catalog's handlers add to a resolved call.
//!
//! The example's messages are hand-written serde types standing in for
//! generated ones, so these helpers name the JSON codec explicitly rather than
//! relying on [`transcode::codec::Payload`], which a generated prost message
//! satisfies and these do not.

use bytes::Bytes;
use http::StatusCode;
use serde::Serialize;
use serde::de::DeserializeOwned;
use transcode::codec::JsonCodec;
use transcode::error::{FieldViolation, Result};
use transcode::gateway::{Call, Reply};

/// Helpers the catalog's handlers share.
pub trait CallExt {
    /// Decodes the body as protojson.
    ///
    /// # Errors
    ///
    /// A 400 naming what could not be decoded.
    fn decode_json<M: DeserializeOwned>(&self) -> Result<M>;

    /// A reply carrying `message` as protojson.
    ///
    /// # Errors
    ///
    /// A 500 when encoding fails.
    fn json<M: Serialize>(&self, status: StatusCode, message: &M) -> Result<Reply>;

    /// An AIP-133 `201 Created` with its `Location`.
    ///
    /// # Errors
    ///
    /// A 500 when encoding fails.
    fn created_json<M: Serialize>(&self, message: &M, location: &str) -> Result<Reply>;

    /// An AIP-135 `204 No Content`, keeping the negotiated content type.
    ///
    /// # Errors
    ///
    /// Never in practice; shares the signature of the others.
    fn deleted(&self) -> Result<Reply>;

    /// A non-negative integer parameter, `0` when absent.
    ///
    /// # Errors
    ///
    /// A 400 when the value is not a number.
    fn query_usize(&self, name: &str) -> Result<usize>;

    /// A boolean parameter: present-and-empty, `true` or `1` is true.
    fn query_bool(&self, name: &str) -> bool;

    /// The AIP-134 `updateMask`, split into paths.
    fn update_mask(&self) -> Vec<String>;
}

impl CallExt for Call<'_> {
    fn decode_json<M: DeserializeOwned>(&self) -> Result<M> {
        self.decode_with(&JsonCodec::new())
    }

    fn json<M: Serialize>(&self, status: StatusCode, message: &M) -> Result<Reply> {
        self.reply_with(&JsonCodec::new(), status, message)
    }

    fn created_json<M: Serialize>(&self, message: &M, location: &str) -> Result<Reply> {
        Ok(self
            .json(StatusCode::CREATED, message)?
            .with_header(http::header::LOCATION, location))
    }

    fn deleted(&self) -> Result<Reply> {
        let mut reply = self.json(StatusCode::NO_CONTENT, &crate::requests::Empty {})?;
        reply.body = Bytes::new();
        Ok(reply)
    }

    fn query_usize(&self, name: &str) -> Result<usize> {
        match self.query.get(name) {
            None => Ok(0),
            Some(raw) => raw.parse().map_err(|_| {
                self.invalid(vec![FieldViolation {
                    field: name.to_string(),
                    description: format!("Expected a number, got {raw:?}."),
                    reason: "INVALID_VALUE".into(),
                }])
            }),
        }
    }

    fn query_bool(&self, name: &str) -> bool {
        matches!(self.query.get(name), Some("true" | "1" | ""))
    }

    fn update_mask(&self) -> Vec<String> {
        self.query
            .get("updateMask")
            .map(|raw| {
                raw.split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// A resource id derived from a display name, for a create that names none.
#[must_use]
pub(super) fn generated_id(source: &str) -> String {
    let slug: String = source
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = slug.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "unnamed".to_string()
    } else {
        trimmed
    }
}
