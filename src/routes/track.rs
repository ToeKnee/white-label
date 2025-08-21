//! Routes for handling track-related operations.

use leptos::prelude::ServerFnError;
use leptos::server;

use crate::forms::track::{CreateTrackForm, UpdateTrackForm};
use crate::models::{artist::Artist, release::Release, track::Track};
#[cfg(feature = "ssr")]
use crate::services::track::{
    create_track_service, delete_track_service, get_track_service, get_tracks_service,
    restore_track_service, update_track_service,
};
#[cfg(feature = "ssr")]
use crate::state::{auth, pool};

/// Contains multiple Tracks.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct TracksResult {
    /// A vector of tracks.
    pub tracks: Vec<Track>,
}

/// The result of fetching a single track along with its associated artists and releases.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct TrackResult {
    /// The track being fetched.
    pub track: Track,
    /// A vector of artists associated with the track.
    pub artists: Vec<Artist>,
    /// A vector of releases associated with the track.
    pub releases: Vec<Release>,
}

/// Get all tracks for a specific artist and release.
///
/// # Arguments:
/// * `artist_slug`: The slug of the artist.
/// * `release_slug`: The slug of the release.
///
/// # Returns:
/// * A `TracksResult` containing a vector of tracks associated with the specified artist and release.
///
/// # Errors:
/// Will return a `ServerFnError` if the artist or release cannot be found, or if there is an issue with the database connection.
#[server(GetTracks, "/api", endpoint = "get_tracks")]
pub async fn get_tracks(
    /// The slug of the artist.
    artist_slug: String,
    /// The slug of the release.
    release_slug: String,
) -> Result<TracksResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    get_tracks_service(&pool, user, artist_slug, release_slug).await
}

/// Get a specific track by its slug, along with its associated artists and releases.
///
/// # Arguments:
/// * `artist_slug`: The slug of the artist.
/// * `release_slug`: The slug of the release.
/// * `slug`: The slug of the track.
///
/// # Returns:
/// * A `TrackResult` containing the track, its associated artists, and releases.
///
/// # Errors:
/// Will return a `ServerFnError` if the track cannot be found or if there is an issue with the database connection.
#[server(GetTrack, "/api", endpoint = "get_track")]
pub async fn get_track(
    /// The slug of the artist.
    artist_slug: String,
    /// The slug of the release.
    release_slug: String,
    /// The slug of the track.
    slug: String,
) -> Result<TrackResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    get_track_service(&pool, user, artist_slug, release_slug, slug).await
}

/// Create a new track with the provided form data.
///
/// # Arguments:
/// * `form`: The form data containing the details of the track to be created.
///
/// # Returns:
/// * A `TrackResult` containing the created track and its associated artists and releases.
///
/// # Errors:
/// Will return a `ServerFnError` if there is an issue with the database connection or if the user is not authenticated.
#[server(CreateTrack, "/api", endpoint = "create_track")]
pub async fn create_track(
    /// The form data containing the details of the track to be created.
    form: CreateTrackForm,
) -> Result<TrackResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    create_track_service(&pool, user, form).await
}

/// Update an existing track with the provided form data.
///
/// # Arguments:
/// * `form`: The form data containing the updated details of the track.
///
/// # Returns:
/// * A `TrackResult` containing the updated track and its associated artists and releases.
///
/// # Errors:
/// Will return a `ServerFnError` if there is an issue with the database connection or if the user is not authenticated.
#[server(UpdateTrack, "/api", endpoint = "update_track")]
pub async fn update_track(
    /// The form data containing the updated details of the track.
    form: UpdateTrackForm,
) -> Result<TrackResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    update_track_service(&pool, user, form).await
}

/// Delete a track by its slug.
///
/// # Arguments:
/// * `slug`: The slug of the track to be deleted.
/// # Returns:
/// * A `TrackResult` containing the deleted track and its associated artists and releases.
///
/// # Errors:
/// Will return a `ServerFnError` if there is an issue with the database connection or if the user is not authenticated.
#[server(DeleteTrack, "/api", endpoint = "delete_track")]
pub async fn delete_track(
    /// The slug of the track to be deleted.
    slug: String,
) -> Result<TrackResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    delete_track_service(&pool, user, slug).await
}

/// Restore a deleted track.
///
/// # Arguments:
/// * `slug`: The slug of the track to be restored.
///
/// # Returns:
/// * A `TrackResult` containing the restored track.
///
/// # Errors:
/// Will return a `ServerFnError` if there is an issue with deleting the track, such as database connection issues or unauthorized access.
#[server(RestoreTrack, "/api", endpoint = "restore_track")]
pub async fn restore_track(
    /// The slug of the track to be restored.
    slug: String,
) -> Result<TrackResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    restore_track_service(&pool, user, slug).await
}
