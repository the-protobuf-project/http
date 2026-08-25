//! The resolved request a method handler receives.

use crate::generated::{DOMAIN, Method};
use crate::store::Catalog;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use transcode::codec::{Decode, JsonCodec};
use transcode::error::{Code, Error};

/// A resolved request, ready for a method handler.
///
/// Captures are already decoded; the body is still bytes, because which codec
/// decodes it was decided by negotiation and the handler knows its own types.
#[derive(Debug)]
pub struct Call<'a> {
    /// The catalog behind the handler.
    pub catalog: &'a Catalog,
    /// Path captures, keyed by protojson field path.
    pub path: HashMap<&'static str, String>,
    /// Query parameters, keyed by protojson field path.
    pub query: HashMap<String, String>,
    /// The raw request body.
    pub body: Vec<u8>,
    /// The method being served, for error metadata.
    pub method: Method,
    /// The request's `Accept` header, for content negotiation.
    pub accept: Option<String>,
    /// The codecs negotiated for this request.
    pub codecs: super::negotiate::Codecs,
}

impl Call<'_> {
    /// A required path capture.
    ///
    /// # Errors
    ///
    /// `INTERNAL` when the capture is absent, which would mean the route table
    /// and the handler disagree — a generator bug rather than a caller error,
    /// so it is not a `400`.
    pub fn capture(&self, field: &str) -> Result<&str, Box<Error>> {
        self.path.get(field).map(String::as_str).ok_or_else(|| {
            Box::new(
                Error::new(Code::Internal, format!("Route did not bind {field:?}."))
                    .with_error_info(
                        "BINDING_MISMATCH",
                        DOMAIN,
                        [("method".into(), self.method.full_name().into())],
                    ),
            )
        })
    }

    /// Rejects any query parameter the binding does not bind.
    ///
    /// This is the opposite of grpc-gateway, which discards them — turning a
    /// typo in an update call into a silent no-op. The parameter is named as
    /// the caller spelled it, because that is what they have to correct.
    ///
    /// The system parameters are always allowed: they select a codec or shape a
    /// response rather than binding to a field.
    ///
    /// # Errors
    ///
    /// `400` with a `BadRequest` naming every unknown parameter at once, so a
    /// caller with two typos learns about two.
    pub fn reject_unknown_query(&self, bound: &[&str]) -> Result<(), Box<Error>> {
        let mut unknown: Vec<&str> = self
            .query
            .keys()
            .map(String::as_str)
            .filter(|name| !bound.contains(name) && !super::query::SYSTEM_PARAMS.contains(name))
            .collect();
        if unknown.is_empty() {
            return Ok(());
        }

        // Sorted so the response is deterministic: the map's iteration order is
        // not, and an error body that reorders between identical requests is
        // one no test can assert on.
        unknown.sort_unstable();

        Err(Box::new(Error::invalid_fields(
            unknown
                .into_iter()
                .map(|name| transcode::error::FieldViolation {
                    field: name.to_string(),
                    description: format!("Unknown query parameter {name:?}."),
                    reason: "UNKNOWN_QUERY_PARAMETER".into(),
                })
                .collect(),
            "UNKNOWN_QUERY_PARAMETER",
            DOMAIN,
            self.method.full_name(),
        )))
    }

    /// A query parameter parsed as a `usize`, or 0 when absent.
    ///
    /// # Errors
    ///
    /// `400` with a `FieldViolation` when the value is not a number, naming the
    /// parameter as the client spelled it.
    pub fn query_usize(&self, name: &str) -> Result<usize, Box<Error>> {
        match self.query.get(name) {
            None => Ok(0),
            Some(raw) => raw.parse().map_err(|_| {
                Box::new(Error::invalid_fields(
                    vec![transcode::error::FieldViolation {
                        field: name.to_string(),
                        description: format!("Expected a number, got {raw:?}."),
                        reason: "INVALID_VALUE".into(),
                    }],
                    "INVALID_ARGUMENT",
                    DOMAIN,
                    self.method.full_name(),
                ))
            }),
        }
    }

    /// The `update_mask` query parameter, split into protojson field paths.
    ///
    /// An absent mask yields an empty list, which AIP-134 defines as "replace
    /// every mutable field".
    pub fn update_mask(&self) -> Vec<String> {
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

impl Call<'_> {
    /// Decodes the request body with the negotiated codec.
    ///
    /// # Errors
    ///
    /// `400` naming the offending field, so a typo is reported rather than
    /// silently ignored — the behaviour README §2 requires and
    /// grpc-gateway does not provide for query parameters.
    pub fn decode<M: DeserializeOwned>(&self) -> Result<M, Box<Error>> {
        JsonCodec::new()
            .decode(&self.body)
            .map_err(|err| Box::new(err.into_gateway_error(DOMAIN, self.method.full_name())))
    }

    /// Maps a `tonic::Status` from the service into the AIP-193 model.
    ///
    /// # Errors
    ///
    /// The mapped status, so a handler can use `?` and never construct an HTTP
    /// status itself.
    pub fn rpc<T>(&self, result: Result<T, tonic::Status>) -> Result<T, Box<Error>> {
        result.map_err(|status| Box::new(Error::from_status(&status, DOMAIN)))
    }

    /// A boolean query parameter, absent meaning false.
    ///
    /// Accepts the protojson spellings plus the bare flag form (`?force`),
    /// which is what a human typing a URL will reach for.
    #[must_use]
    pub fn query_bool(&self, name: &str) -> bool {
        match self.query.get(name).map(String::as_str) {
            Some("true" | "1" | "") => true,
            Some(_) | None => false,
        }
    }

    /// Derives a resource id from a display name, for a Create that did not
    /// supply one.
    ///
    /// A real service would use a random id or the caller's `*_id` field; this
    /// is deterministic so the tests can predict it.
    #[must_use]
    pub fn generated_id(&self, source: &str) -> String {
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
}
