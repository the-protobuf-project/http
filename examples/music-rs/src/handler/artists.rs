//! Artist method handlers.
//!
//! Each is the shape `protoc-gen-http` will emit: bind from the path, decode
//! the body with the negotiated codec, call the service, encode the response.

use super::{Call, Reply};
use crate::model::Artist;
use crate::requests::{Empty, ListArtistsResponse};
use http::StatusCode;
use transcode::error::Error;

/// `GET /v1/{name=artists/*}`
pub(super) fn get(call: &Call<'_>) -> Result<Reply, Box<Error>> {
    let name = call.capture("name")?;
    let artist = call.rpc(call.catalog.get_artist(name))?;
    call.ok(&artist)
}

/// `GET /v1/artists`
pub(super) fn list(call: &Call<'_>) -> Result<Reply, Box<Error>> {
    call.reject_unknown_query(&["pageSize", "pageToken"])?;
    let page_size = call.query_usize("pageSize")?;
    let artists = call.rpc(call.catalog.list_artists(page_size))?;
    call.ok(&ListArtistsResponse {
        artists,
        next_page_token: String::new(),
    })
}

/// `POST /v1/artists` with `body: "artist"`.
///
/// AIP-133: a Create returns `201` with a `Location` header naming the created
/// resource, which is what lets a client follow the response without knowing
/// how names are formed.
pub(super) fn create(call: &Call<'_>) -> Result<Reply, Box<Error>> {
    let mut artist: Artist = call.decode()?;
    if artist.name.is_empty() {
        artist.name = format!("artists/{}", call.generated_id(&artist.display_name));
    }
    let created = call.rpc(call.catalog.create_artist(artist))?;
    call.created(&created, &format!("/v1/{}", created.name))
}

/// `PATCH /v1/{artist.name=artists/*}` with `body: "artist"`.
pub(super) fn update(call: &Call<'_>) -> Result<Reply, Box<Error>> {
    call.reject_unknown_query(&["updateMask"])?;
    let name = call.capture("artist.name")?.to_string();
    let patch: Artist = call.decode()?;
    let updated = call.rpc(
        call.catalog
            .update_artist(&name, patch, &call.update_mask()),
    )?;
    call.ok(&updated)
}

/// `DELETE /v1/{name=artists/*}`
///
/// AIP-135's `force` decides whether child tracks go with it; without it, an
/// artist that still has tracks is a `FAILED_PRECONDITION` rather than a
/// silent cascade.
pub(super) fn delete(call: &Call<'_>) -> Result<Reply, Box<Error>> {
    call.reject_unknown_query(&["force"])?;
    let name = call.capture("name")?;
    let force = call.query_bool("force");
    call.rpc(call.catalog.delete_artist(name, force))?;

    // google.protobuf.Empty with no response_body is 204, per README §4
    let mut reply = call.ok(&Empty {})?;
    reply.status = StatusCode::NO_CONTENT;
    reply.body.clear();
    Ok(reply)
}
