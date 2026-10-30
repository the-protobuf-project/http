//! Track method handlers.

use super::call::{CallExt, generated_id};
use crate::model::Track;
use crate::requests::{ListTracksResponse, WithdrawTrackBody};
use crate::store::Catalog;
use http::StatusCode;
use transcode::error::Result;
use transcode::gateway::{Call, Reply};

/// `GET /v1/{name=artists/*/tracks/*}`
pub(super) fn get(catalog: &Catalog, call: &Call<'_>) -> Result<Reply> {
    let name = call.capture("name")?;
    let track = call.rpc(catalog.get_track(name))?;
    call.json(StatusCode::OK, &track)
}

/// `GET /v1/{parent=artists/*}/tracks`
pub(super) fn list(catalog: &Catalog, call: &Call<'_>) -> Result<Reply> {
    call.reject_unknown_query(&["pageSize", "pageToken"])?;
    let parent = call.capture("parent")?;
    let page_size = call.query_usize("pageSize")?;
    let tracks = call.rpc(catalog.list_tracks(parent, page_size))?;
    call.json(
        StatusCode::OK,
        &ListTracksResponse {
            tracks,
            next_page_token: String::new(),
        },
    )
}

/// `POST /v1/{parent=artists/*}/tracks`
pub(super) fn create(catalog: &Catalog, call: &Call<'_>) -> Result<Reply> {
    let parent = call.capture("parent")?.to_string();
    let mut track: Track = call.decode_json()?;
    if track.name.is_empty() {
        track.name = format!("{parent}/tracks/{}", generated_id(&track.title));
    }
    let created = call.rpc(catalog.create_track(&parent, track))?;
    call.created_json(&created, &format!("/v1/{}", created.name))
}

/// `PATCH /v1/{track.name=artists/*/tracks/*}`
pub(super) fn update(catalog: &Catalog, call: &Call<'_>) -> Result<Reply> {
    call.reject_unknown_query(&["updateMask"])?;
    let name = call.capture("track.name")?.to_string();
    let patch: Track = call.decode_json()?;
    let updated = call.rpc(catalog.update_track(&name, patch, &call.update_mask()))?;
    call.json(StatusCode::OK, &updated)
}

/// `DELETE /v1/{name=artists/*/tracks/*}`, answering `204`.
pub(super) fn delete(catalog: &Catalog, call: &Call<'_>) -> Result<Reply> {
    let name = call.capture("name")?;
    call.rpc(catalog.delete_track(name))?;
    call.deleted()
}

/// `POST /v1/{name=artists/*/tracks/*}:withdraw` — an AIP-136 custom method.
pub(super) fn withdraw(catalog: &Catalog, call: &Call<'_>) -> Result<Reply> {
    let name = call.capture("name")?.to_string();
    // Decoded for its validation: an unknown field in the body is a 400 even
    // though the reason is not stored.
    let _body: WithdrawTrackBody = call.decode_json()?;
    let withdrawn = call.rpc(catalog.withdraw_track(&name))?;
    call.json(StatusCode::OK, &withdrawn)
}
