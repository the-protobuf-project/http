//! Track method handlers.

use super::{Call, Reply};
use crate::model::Track;
use crate::requests::{Empty, ListTracksResponse, WithdrawTrackBody};
use http::StatusCode;
use transcode::error::Error;

/// `GET /v1/{name=artists/*/tracks/*}` — the multi-segment capture.
pub(super) fn get(call: &Call<'_>) -> Result<Reply, Box<Error>> {
    let name = call.capture("name")?;
    let track = call.rpc(call.catalog.get_track(name))?;
    call.ok(&track)
}

/// `GET /v1/{parent=artists/*}/tracks` — a capture followed by a literal.
pub(super) fn list(call: &Call<'_>) -> Result<Reply, Box<Error>> {
    call.reject_unknown_query(&["pageSize", "pageToken"])?;
    let parent = call.capture("parent")?;
    let page_size = call.query_usize("pageSize")?;
    let tracks = call.rpc(call.catalog.list_tracks(parent, page_size))?;
    call.ok(&ListTracksResponse {
        tracks,
        next_page_token: String::new(),
    })
}

/// `POST /v1/{parent=artists/*}/tracks` with `body: "track"`.
pub(super) fn create(call: &Call<'_>) -> Result<Reply, Box<Error>> {
    let parent = call.capture("parent")?.to_string();
    let mut track: Track = call.decode()?;
    if track.name.is_empty() {
        track.name = format!("{parent}/tracks/{}", call.generated_id(&track.title));
    }
    let created = call.rpc(call.catalog.create_track(&parent, track))?;
    call.created(&created, &format!("/v1/{}", created.name))
}

/// `PATCH /v1/{track.name=artists/*/tracks/*}` with `body: "track"`.
pub(super) fn update(call: &Call<'_>) -> Result<Reply, Box<Error>> {
    call.reject_unknown_query(&["updateMask"])?;
    let name = call.capture("track.name")?.to_string();
    let patch: Track = call.decode()?;
    let updated = call.rpc(call.catalog.update_track(&name, patch, &call.update_mask()))?;
    call.ok(&updated)
}

/// `DELETE /v1/{name=artists/*/tracks/*}`
pub(super) fn delete(call: &Call<'_>) -> Result<Reply, Box<Error>> {
    let name = call.capture("name")?;
    call.rpc(call.catalog.delete_track(name))?;

    let mut reply = call.ok(&Empty {})?;
    reply.status = StatusCode::NO_CONTENT;
    reply.body.clear();
    Ok(reply)
}

/// `POST /v1/{name=artists/*/tracks/*}:withdraw` with `body: "*"`.
///
/// The custom verb is the case a general-purpose router mis-binds: `matchit`
/// accepts this template and folds `:withdraw` into `name`, so the handler
/// would look up a track literally named `…/tracks/t1:withdraw`.
pub(super) fn withdraw(call: &Call<'_>) -> Result<Reply, Box<Error>> {
    let name = call.capture("name")?.to_string();

    // `body: "*"` means the whole message, but `name` came from the path, and
    // a field bound by the path must not also appear in the body.
    let _body: WithdrawTrackBody = call.decode()?;

    let withdrawn = call.rpc(call.catalog.withdraw_track(&name))?;
    call.ok(&withdrawn)
}
