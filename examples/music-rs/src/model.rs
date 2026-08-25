//! The catalog's messages.
//!
//! Hand-written here, but exactly what `prost` plus generated protojson
//! `Serialize` impls will produce: `#[serde(rename_all = "camelCase")]`,
//! `deny_unknown_fields` so a typo is rejected rather than ignored, and
//! `int64` rendered as a JSON **string** per README §4.1
//!
//! Writing them by hand is what lets the runtime be proved end to end before
//! `protoc-gen-http` exists. Every choice here is one the generator will make.

use serde::{Deserialize, Serialize};

/// An artist in the catalog. Resource `music.example.com/Artist`, pattern
/// `artists/{artist}`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Artist {
    /// The resource name, `artists/{artist}`. `IDENTIFIER`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,

    /// The name shown to listeners. `REQUIRED`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub display_name: String,

    /// A short biography. `OPTIONAL`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub biography: String,

    /// Listeners in the trailing 30 days. `OUTPUT_ONLY`.
    ///
    /// A `String` because protojson renders every 64-bit integer as a JSON
    /// string: a `double` cannot hold the full `int64` range, so a number here
    /// would lose precision silently.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub monthly_listeners: String,

    /// When the artist was added, RFC 3339. `OUTPUT_ONLY`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub create_time: String,

    /// Opaque concurrency token. `OUTPUT_ONLY`. (AIP-154)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub etag: String,
}

/// A recording. Resource `music.example.com/Track`, pattern
/// `artists/{artist}/tracks/{track}`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Track {
    /// The resource name, `artists/{artist}/tracks/{track}`. `IDENTIFIER`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,

    /// The track title. `REQUIRED`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,

    /// How long the recording runs. `OPTIONAL`.
    ///
    /// A `google.protobuf.Duration`, which protojson renders as decimal seconds
    /// with an `s` suffix — `"545s"`, `"1.5s"`. A `String` here for the same
    /// reason `monthly_listeners` is one: the JSON shape is the contract.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub duration: String,

    /// Whether the recording carries an explicit-content advisory.
    #[serde(default, skip_serializing_if = "is_false")]
    pub explicit: bool,

    /// How the track is distributed. `OPTIONAL`.
    #[serde(default, skip_serializing_if = "Availability::is_unspecified")]
    pub availability: Availability,

    /// When the track was added, RFC 3339. `OUTPUT_ONLY`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub create_time: String,

    /// Idempotency key. `OPTIONAL`. (AIP-155)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub request_id: String,
}

/// How a track is distributed.
///
/// Serialized as its proto enum **name**, not its number, per protojson.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Availability {
    /// Unspecified.
    #[default]
    #[serde(rename = "AVAILABILITY_UNSPECIFIED")]
    Unspecified,
    /// Streamable anywhere.
    #[serde(rename = "AVAILABILITY_STREAMING")]
    Streaming,
    /// Available for purchase only.
    #[serde(rename = "AVAILABILITY_DOWNLOAD_ONLY")]
    DownloadOnly,
    /// Withdrawn from distribution.
    #[serde(rename = "AVAILABILITY_UNAVAILABLE")]
    Unavailable,
}

impl Availability {
    /// Whether the value is at its protojson default, which is omitted.
    fn is_unspecified(&self) -> bool {
        *self == Availability::Unspecified
    }
}

/// Whether a `bool` is at its default.
fn is_false(v: &bool) -> bool {
    !*v
}
