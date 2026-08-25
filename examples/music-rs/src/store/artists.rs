//! Artist CRUD.

use super::{Catalog, FIXED_TIME};
use crate::model::Artist;
use tonic::Status;

impl Catalog {
    /// Returns one artist. (AIP-131)
    ///
    /// # Errors
    ///
    /// `NOT_FOUND` when no artist has that name.
    pub fn get_artist(&self, name: &str) -> Result<Artist, Status> {
        self.lock()?
            .artists
            .get(name)
            .cloned()
            .ok_or_else(|| not_found(name))
    }

    /// Lists artists, ordered by resource name. (AIP-132)
    ///
    /// # Errors
    ///
    /// `INTERNAL` if the catalog lock is poisoned.
    pub fn list_artists(&self, page_size: usize) -> Result<Vec<Artist>, Status> {
        let inner = self.lock()?;
        let limit = if page_size == 0 { 50 } else { page_size };
        Ok(inner.artists.values().take(limit).cloned().collect())
    }

    /// Creates an artist. (AIP-133)
    ///
    /// Server-assigned fields are set here, and any value the caller sent for
    /// them was already rejected upstream by the `OUTPUT_ONLY` rule.
    ///
    /// # Errors
    ///
    /// `ALREADY_EXISTS` when the name is taken, `INVALID_ARGUMENT` when
    /// `display_name` is missing.
    pub fn create_artist(&self, mut artist: Artist) -> Result<Artist, Status> {
        if artist.display_name.is_empty() {
            return Err(Status::invalid_argument("display_name is required"));
        }
        let etag = self.next_etag();
        let mut inner = self.lock()?;

        if inner.artists.contains_key(&artist.name) {
            return Err(Status::already_exists(format!(
                "Artist {:?} already exists.",
                artist.name
            )));
        }
        artist.create_time = FIXED_TIME.to_string();
        artist.etag = etag;
        inner.artists.insert(artist.name.clone(), artist.clone());
        Ok(artist)
    }

    /// Updates an artist. (AIP-134)
    ///
    /// `update_mask` names the fields to change; an empty mask replaces every
    /// mutable field, which is what an absent mask means.
    ///
    /// # Errors
    ///
    /// `NOT_FOUND` when the artist does not exist.
    pub fn update_artist(
        &self,
        name: &str,
        patch: Artist,
        update_mask: &[String],
    ) -> Result<Artist, Status> {
        let etag = self.next_etag();
        let mut inner = self.lock()?;
        let current = inner.artists.get_mut(name).ok_or_else(|| not_found(name))?;

        let touches =
            |field: &str| update_mask.is_empty() || update_mask.iter().any(|m| m == field);
        if touches("displayName") {
            current.display_name = patch.display_name;
        }
        if touches("biography") {
            current.biography = patch.biography;
        }
        current.etag = etag;
        Ok(current.clone())
    }

    /// Deletes an artist, and its tracks when `force` is set. (AIP-135)
    ///
    /// # Errors
    ///
    /// `NOT_FOUND` when the artist does not exist, `FAILED_PRECONDITION` when
    /// it still has tracks and `force` is false.
    pub fn delete_artist(&self, name: &str, force: bool) -> Result<(), Status> {
        let mut inner = self.lock()?;
        if !inner.artists.contains_key(name) {
            return Err(not_found(name));
        }

        let prefix = format!("{name}/tracks/");
        let has_children = inner.tracks.keys().any(|key| key.starts_with(&prefix));

        // AIP-135: deleting a parent that still has children needs `force`.
        // Cascading silently would destroy data the caller did not name.
        if has_children && !force {
            return Err(Status::failed_precondition(format!(
                "Artist {name:?} still has tracks; set force=true to delete them too."
            )));
        }
        inner.artists.remove(name);
        inner.tracks.retain(|key, _| !key.starts_with(&prefix));
        Ok(())
    }

    /// Whether an artist exists, for validating a `parent` reference.
    ///
    /// # Errors
    ///
    /// `INTERNAL` if the catalog lock is poisoned.
    pub fn artist_exists(&self, name: &str) -> Result<bool, Status> {
        Ok(self.lock()?.artists.contains_key(name))
    }
}

/// The `NOT_FOUND` status for an artist, carrying the name so the AIP-193
/// envelope can report what was looked for.
fn not_found(name: &str) -> Status {
    Status::not_found(format!("Artist {name:?} not found."))
}
