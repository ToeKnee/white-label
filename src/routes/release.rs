//! Routes for managing releases in the application.

use leptos::prelude::ServerFnError;
use leptos::server;

use crate::forms::release::{CreateReleaseForm, UpdateReleaseForm};
#[cfg(feature = "ssr")]
use crate::models::record_label::RecordLabel;
use crate::models::{artist::Artist, release::Release, track_with_artists::TrackWithArtists};
#[cfg(feature = "ssr")]
use crate::services::release::{
    create_release_service, delete_release_service, get_next_scheduled_release_service,
    get_release_service, get_releases_service, restore_release_service, update_release_service,
};
#[cfg(feature = "ssr")]
use crate::state::{auth, pool};

/// Contains multiple Releases.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct ReleasesResult {
    /// A vector of releases.
    pub releases: Vec<Release>,
}

/// The result of fetching a single release along with its associated artists and tracks.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct ReleaseResult {
    /// The release being fetched.
    pub release: Release,
    /// A vector of artists associated with the release.
    pub artists: Vec<Artist>,
    /// A vector of tracks with artists associated with the release.
    pub tracks: Vec<TrackWithArtists>,
}

/// Get all releases for a specific artist.
///
/// # Arguments:
/// * `artist_slug`: The slug of the artist.
///
/// # Returns:
/// * A `ReleasesResult` containing a vector of releases associated with the specified artist.
///
/// # Errors:
/// Will return a `ServerFnError` if the artist cannot be found, or if there is an issue with the database connection.
#[server(GetReleases, "/api", endpoint = "get_releases")]
pub async fn get_releases(
    /// The slug of the artist.
    artist_slug: String,
) -> Result<ReleasesResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth().await?;
    let user = auth.current_user.as_ref();
    get_releases_service(&pool, user, artist_slug).await
}

/// Get the next scheduled release for a specific artist.
///
/// # Arguments:
/// * `artist_slug`: An optional slug of the artist. If provided, it will filter releases for that artist.
///
/// # Returns:
/// * An `Option<ReleaseResult>` containing the next scheduled release for the specified artist, or `None` if no such release exists.
///
/// # Errors:
/// Will return a `ServerFnError` if there is an issue with the database connection or if the record label cannot be retrieved.
#[server(
    GetNextScheduledRelease,
    "/api",
    endpoint = "get_next_scheduled_release"
)]
pub async fn get_next_scheduled_release(
    /// The slug of the artist, if provided.
    artist_slug: Option<String>,
) -> Result<Option<ReleaseResult>, ServerFnError> {
    let pool = pool()?;
    let record_label = RecordLabel::first(&pool)
        .await
        .map_err(|_| ServerFnError::new("Failed to retrieve record label"))?;
    let artist_id = match artist_slug {
        Some(slug) if slug.is_empty() => match Artist::get_by_slug(&pool, slug).await {
            Ok(artist) => Some(artist.id),
            Err(_) => None,
        },
        _ => None,
    };
    get_next_scheduled_release_service(&pool, artist_id, record_label.id).await
}

/// Get a specific release by its slug, along with its associated artists and tracks.
///
/// # Arguments:
/// * `artist_slug`: The slug of the artist.
/// * `slug`: The slug of the release.
///
/// # Returns:
/// * A `ReleaseResult` containing the release, its associated artists, and tracks.
///
/// # Errors:
/// Will return a `ServerFnError` if the release cannot be found or if there is an issue with the database connection.
#[server(GetRelease, "/api", endpoint = "get_release")]
pub async fn get_release(
    /// The slug of the artist.
    artist_slug: String,
    /// The slug of the release.
    slug: String,
) -> Result<ReleaseResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth().await?;
    let user = auth.current_user.as_ref();
    get_release_service(&pool, user, artist_slug, slug).await
}

/// Create a new release with the provided form data.
///
/// # Arguments:
/// * `form`: The form data containing the details of the release to be created.
///
/// # Returns:
/// * A `ReleaseResult` containing the created release and its associated artists and tracks.
///
/// # Errors:
/// Will return a `ServerFnError` if the release cannot be created, or if there is an issue with the database connection.
#[server(CreateRelease, "/api", endpoint = "create_release")]
pub async fn create_release(
    /// The form data for creating a new release.
    form: CreateReleaseForm,
) -> Result<ReleaseResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth().await?;
    let user = auth.current_user.as_ref();
    create_release_service(&pool, user, form).await
}

/// Update an existing release with the provided form data.
///
/// # Arguments:
/// * `form`: The form data containing the updated details of the release.
///
/// # Returns:
/// * A `ReleaseResult` containing the updated release and its associated artists and tracks.
///
/// # Errors:
/// Will return a `ServerFnError` if the release cannot be updated, or if there is an issue with the database connection.
#[server(UpdateRelease, "/api", endpoint = "update_release")]
pub async fn update_release(
    /// The form data for updating an existing release.
    form: UpdateReleaseForm,
) -> Result<ReleaseResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth().await?;
    let user = auth.current_user.as_ref();
    update_release_service(&pool, user, form).await
}

/// Delete a release by its slug.
///
/// # Arguments:
/// * `slug`: The slug of the release to be deleted.
///
/// # Returns:
/// * A `ReleaseResult` containing the deleted release and its associated artists and tracks.
///
/// # Errors:
/// Will return a `ServerFnError` if the release cannot be deleted, or if there is an issue with the database connection.
#[server(DeleteRelease, "/api", endpoint = "delete_release")]
pub async fn delete_release(
    /// The slug of the release to be deleted.
    slug: String,
) -> Result<ReleaseResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth().await?;
    let user = auth.current_user.as_ref();
    delete_release_service(&pool, user, slug).await
}

/// Restore a deleted release.
///
/// # Arguments:
/// * `slug`: The slug of the release to be restored.
///
/// # Returns:
/// * A `ReleaseResult` containing the restored release.
///
/// # Errors:
/// Will return a `ServerFnError` if there is an issue with deleting the release, such as database connection issues or unauthorized access.
#[server(RestoreRelease, "/api", endpoint = "restore_release")]
pub async fn restore_release(
    /// The slug of the release to be restored.
    slug: String,
) -> Result<ReleaseResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth().await?;
    let user = auth.current_user.as_ref();
    restore_release_service(&pool, user, slug).await
}
