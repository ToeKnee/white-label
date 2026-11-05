//! Routes for managing music services for an artist.

use leptos::prelude::ServerFnError;
use leptos::server;

use crate::forms::links::LinksForm;
#[cfg(feature = "ssr")]
use crate::services::links::{get_links_service, update_links_service};
#[cfg(feature = "ssr")]
use crate::state::{auth, pool};

pub use crate::api_types::links::LinksResult;

/// Get all music services and social links for a specific artist.
///
/// # Arguments:
/// * `artist_slug`: The slug of the artist.
///
/// # Returns:
/// * A `MusicServicesResult` containing a vector of music services associated with the specified artist.
///
/// # Errors:
/// Will return a `ServerFnError` if the artist cannot be found, or if there is an issue with the database connection.
#[server(GetLinks, "/api", endpoint = "get_links")]
pub async fn get_links(
    /// The slug of the artist.
    artist_slug: String,
) -> Result<LinksResult, ServerFnError> {
    let pool = pool()?;
    get_links_service(&pool, artist_slug)
        .await
        .map_err(ServerFnError::from)
}

/// Update music services social median links for an artist.
///
/// # Arguments:
/// * `artist_slug`: The slug of the artist.
#[server(UpdateLinks, "/api", endpoint = "update_links")]
pub async fn update_links(
    /// The form containing the music service and social links to update.
    form: LinksForm,
) -> Result<LinksResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth().await?;
    let user = auth.current_user.as_ref();

    tracing::info!("Updating links for artist: {}", form.artist_slug);
    tracing::info!("From: {:?}", form);

    update_links_service(&pool, user, form)
        .await
        .map_err(ServerFnError::from)
}
