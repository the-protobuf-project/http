//! Request and response messages for the Catalog service.

use crate::model::{Artist, Track};
use serde::{Deserialize, Serialize};

/// Body of `CreateArtist`: the rule declares `body: "artist"`, so the body is
/// the artist itself rather than a wrapper.
pub type CreateArtistBody = Artist;

/// Body of `CreateTrack`, for the same reason.
pub type CreateTrackBody = Track;

/// Response for `ListArtists`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListArtistsResponse {
    /// The artists in the catalog.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artists: Vec<Artist>,

    /// A token for the next page, or empty if this is the last.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub next_page_token: String,
}

/// Response for `ListTracks`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListTracksResponse {
    /// The artist's tracks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tracks: Vec<Track>,

    /// A token for the next page, or empty if this is the last.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub next_page_token: String,
}

/// Body of `WithdrawTrack`.
///
/// The rule declares `body: "*"`, so the whole request message is the body —
/// but `name` is bound from the path, and a field bound by the path must not
/// also appear in the body (README §2).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WithdrawTrackBody {
    /// Why the track is being withdrawn.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason: String,
}

/// `google.protobuf.Empty`, which protojson renders as `{}`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Empty {}

/// Response for `WatchTracks`.
///
/// A custom method returns its own response message (AIP-136), which also
/// leaves room to carry stream-level metadata later without breaking clients.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchTracksResponse {
    /// The track that was added or changed.
    pub track: crate::model::Track,
}
