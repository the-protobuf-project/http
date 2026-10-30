//! Artist method handlers.
//!
//! Each is the shape `protoc-gen-http` will emit: bind from the path, decode
//! the body, call the service, encode the response.

use super::call::{CallExt, generated_id};
use crate::model::Artist;
use crate::requests::ListArtistsResponse;
use crate::store::Catalog;
use http::StatusCode;
use transcode::error::Result;
use transcode::gateway::{Call, Reply};

/// `GET /v1/{name=artists/*}`
pub(super) fn get(catalog: &Catalog, call: &Call<'_>) -> Result<Reply> {
    let name = call.capture("name")?;
    let artist = call.rpc(catalog.get_artist(name))?;
    call.json(StatusCode::OK, &artist)
}

/// `GET /v1/artists`
pub(super) fn list(catalog: &Catalog, call: &Call<'_>) -> Result<Reply> {
    call.reject_unknown_query(&["pageSize", "pageToken"])?;
    let page_size = call.query_usize("pageSize")?;
    let artists = call.rpc(catalog.list_artists(page_size))?;
    call.json(
        StatusCode::OK,
        &ListArtistsResponse {
            artists,
            next_page_token: String::new(),
        },
    )
}

/// `POST /v1/artists`, answering AIP-133's `201` with a `Location`.
pub(super) fn create(catalog: &Catalog, call: &Call<'_>) -> Result<Reply> {
    let mut artist: Artist = call.decode_json()?;
    if artist.name.is_empty() {
        artist.name = format!("artists/{}", generated_id(&artist.display_name));
    }
    let created = call.rpc(catalog.create_artist(artist))?;
    call.created_json(&created, &format!("/v1/{}", created.name))
}

/// `PATCH /v1/{artist.name=artists/*}`
pub(super) fn update(catalog: &Catalog, call: &Call<'_>) -> Result<Reply> {
    call.reject_unknown_query(&["updateMask"])?;
    let name = call.capture("artist.name")?.to_string();
    let patch: Artist = call.decode_json()?;
    let updated = call.rpc(catalog.update_artist(&name, patch, &call.update_mask()))?;
    call.json(StatusCode::OK, &updated)
}

/// `DELETE /v1/{name=artists/*}`, answering `204`.
pub(super) fn delete(catalog: &Catalog, call: &Call<'_>) -> Result<Reply> {
    call.reject_unknown_query(&["force"])?;
    let name = call.capture("name")?;
    let force = call.query_bool("force");
    call.rpc(catalog.delete_artist(name, force))?;
    call.deleted()
}
