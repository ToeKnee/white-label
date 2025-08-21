//! Routes for managing artists in the application.
use leptos::prelude::ServerFnError;
use leptos::server;

use crate::forms::artist::{CreateArtistForm, UpdateArtistForm};
use crate::models::artist::Artist;

#[cfg(feature = "ssr")]
use crate::services::artist::{
    create_artist_service, delete_artist_service, get_artist_service, restore_artist_service,
    update_artist_service,
};
#[cfg(feature = "ssr")]
use crate::state::{auth, pool};

/// Contains the result of fetching a single artist.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct ArtistResult {
    /// The artist being fetched.
    pub artist: Artist,
}

/// Get a specific artist by its slug.
///
/// # Arguments:
/// * `slug`: The slug of the artist.
///
/// # Returns:
/// * A `ArtistResult` containing the artist associated with the specified slug.
///
/// # Errors:
/// Will return a `ServerFnError` if the artist cannot be found, or if there is an issue with the database connection.
#[server(GetArtist, "/api", endpoint = "get_artist")]
pub async fn get_artist(
    /// The slug of the artist to fetch.
    slug: String,
) -> Result<ArtistResult, ServerFnError> {
    let pool = pool()?;
    get_artist_service(&pool, slug).await
}

/// Create a new artist.
///
/// # Arguments:
/// * `artist_form`: A `CreateArtistForm` containing the details of the artist to be created.
///
/// # Returns:
/// * A `ArtistResult` containing the newly created artist.
///
/// # Errors:
/// Will return a `ServerFnError` if there is an issue with creating the artist, such as invalid form data or database connection issues.
#[server(CreateArtist, "/api", endpoint = "create_artist")]
pub async fn create_artist(
    /// The form containing the details of the artist to be created.
    artist_form: CreateArtistForm,
) -> Result<ArtistResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    create_artist_service(&pool, user, artist_form).await
}

/// Update an existing artist.
///
/// # Arguments:
/// * `artist_form`: An `UpdateArtistForm` containing the details of the artist to be updated.
///
/// # Returns:
/// * A `ArtistResult` containing the updated artist.
///
/// # Errors:
/// Will return a `ServerFnError` if there is an issue with updating the artist, such as invalid form data or database connection issues.
#[server(UpdateArtist, "/api", endpoint = "update_artist")]
pub async fn update_artist(
    /// The form containing the details of the artist to be updated.
    artist_form: UpdateArtistForm,
) -> Result<ArtistResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    update_artist_service(&pool, user, artist_form).await
}

/// Delete an existing artist.
///
/// # Arguments:
/// * `slug`: The slug of the artist to be deleted.
///
/// # Returns:
/// * A `ArtistResult` containing the deleted artist.
///
/// # Errors:
/// Will return a `ServerFnError` if there is an issue with deleting the artist, such as database connection issues or unauthorized access.
#[server(DeleteArtist, "/api", endpoint = "delete_artist")]
pub async fn delete_artist(
    /// The slug of the artist to be deleted.
    slug: String,
) -> Result<ArtistResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    delete_artist_service(&pool, user, slug).await
}

/// Restore a deleted artist.
///
/// # Arguments:
/// * `slug`: The slug of the artist to be restored.
///
/// # Returns:
/// * A `ArtistResult` containing the restored artist.
///
/// # Errors:
/// Will return a `ServerFnError` if there is an issue with deleting the artist, such as database connection issues or unauthorized access.
#[server(RestoreArtist, "/api", endpoint = "restore_artist")]
pub async fn restore_artist(
    /// The slug of the artist to be restored.
    slug: String,
) -> Result<ArtistResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    restore_artist_service(&pool, user, slug).await
}
