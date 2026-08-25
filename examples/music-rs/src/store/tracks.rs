//! Track CRUD.

use super::{Catalog, FIXED_TIME};
use crate::model::{Availability, Track};
use tonic::Status;

impl Catalog {
    /// Returns one track. (AIP-131)
    ///
    /// # Errors
    ///
    /// `NOT_FOUND` when no track has that name.
    pub fn get_track(&self, name: &str) -> Result<Track, Status> {
        self.lock()?
            .tracks
            .get(name)
            .cloned()
            .ok_or_else(|| not_found(name))
    }

    /// Lists an artist's tracks, ordered by resource name. (AIP-132)
    ///
    /// # Errors
    ///
    /// `NOT_FOUND` when the parent artist does not exist. Returning an empty
    /// list instead would tell a caller their typo'd artist simply has no
    /// tracks, which is the wrong answer.
    pub fn list_tracks(&self, parent: &str, page_size: usize) -> Result<Vec<Track>, Status> {
        if !self.artist_exists(parent)? {
            return Err(Status::not_found(format!("Artist {parent:?} not found.")));
        }
        let inner = self.lock()?;
        let prefix = format!("{parent}/tracks/");
        let limit = if page_size == 0 { 50 } else { page_size };

        Ok(inner
            .tracks
            .range(prefix.clone()..)
            .take_while(|(key, _)| key.starts_with(&prefix))
            .map(|(_, track)| track.clone())
            .take(limit)
            .collect())
    }

    /// Creates a track under an artist. (AIP-133)
    ///
    /// # Errors
    ///
    /// `NOT_FOUND` when the parent does not exist, `ALREADY_EXISTS` when the
    /// name is taken, `INVALID_ARGUMENT` when `title` is missing.
    pub fn create_track(&self, parent: &str, mut track: Track) -> Result<Track, Status> {
        if track.title.is_empty() {
            return Err(Status::invalid_argument("title is required"));
        }
        if !self.artist_exists(parent)? {
            return Err(Status::not_found(format!("Artist {parent:?} not found.")));
        }
        let mut inner = self.lock()?;

        if inner.tracks.contains_key(&track.name) {
            return Err(Status::already_exists(format!(
                "Track {:?} already exists.",
                track.name
            )));
        }
        track.create_time = FIXED_TIME.to_string();
        inner.tracks.insert(track.name.clone(), track.clone());
        Ok(track)
    }

    /// Updates a track. (AIP-134)
    ///
    /// # Errors
    ///
    /// `NOT_FOUND` when the track does not exist.
    pub fn update_track(
        &self,
        name: &str,
        patch: Track,
        update_mask: &[String],
    ) -> Result<Track, Status> {
        let mut inner = self.lock()?;
        let current = inner.tracks.get_mut(name).ok_or_else(|| not_found(name))?;

        let touches =
            |field: &str| update_mask.is_empty() || update_mask.iter().any(|m| m == field);
        if touches("title") {
            current.title = patch.title;
        }
        if touches("duration") {
            current.duration = patch.duration;
        }
        if touches("explicit") {
            current.explicit = patch.explicit;
        }
        if touches("availability") {
            current.availability = patch.availability;
        }
        Ok(current.clone())
    }

    /// Deletes a track. (AIP-135)
    ///
    /// # Errors
    ///
    /// `NOT_FOUND` when the track does not exist.
    pub fn delete_track(&self, name: &str) -> Result<(), Status> {
        self.lock()?
            .tracks
            .remove(name)
            .map(|_| ())
            .ok_or_else(|| not_found(name))
    }

    /// Withdraws a track from distribution. (AIP-136)
    ///
    /// A custom method because it is a state transition, not a field edit: a
    /// caller should not be able to reach it with a `PATCH` of `availability`.
    ///
    /// # Errors
    ///
    /// `NOT_FOUND` when the track does not exist, `FAILED_PRECONDITION` when it
    /// is already withdrawn.
    pub fn withdraw_track(&self, name: &str) -> Result<Track, Status> {
        let mut inner = self.lock()?;
        let current = inner.tracks.get_mut(name).ok_or_else(|| not_found(name))?;

        if current.availability == Availability::Unavailable {
            return Err(Status::failed_precondition(format!(
                "Track {name:?} is already withdrawn."
            )));
        }
        current.availability = Availability::Unavailable;
        Ok(current.clone())
    }
}

/// The `NOT_FOUND` status for a track.
fn not_found(name: &str) -> Status {
    Status::not_found(format!("Track {name:?} not found."))
}
