//! Routes for artists.

use leptos::prelude::ServerFnError;
use leptos::server;

#[cfg(feature = "ssr")]
use crate::services::artists::get_releases_for_artist_service;
#[cfg(feature = "ssr")]
use crate::state::{auth, pool};

pub use crate::api_types::artists::ArtistsReleaseResult;

/// Get releases for multiple artists by their IDs.
///
/// # Arguments:
/// * `artist_ids`: A comma-separated string of artist IDs.
///
/// # Returns:
/// * An `ArtistsReleaseResult` containing a vector of releases associated with the specified artist IDs.
///
/// # Errors:
/// Will return a `ServerFnError` if there is an issue with the database connection or if the releases cannot be retrieved.
#[server(GetArtistsReleases, "/api", endpoint = "artists_releases")]
pub async fn get_releases_for_artist(
    /// The artist ID
    artist_id: i64,
) -> Result<ArtistsReleaseResult, ServerFnError> {
    let auth = auth().await?;
    let pool = pool()?;
    let user = auth.current_user.as_ref();

    let releases = get_releases_for_artist_service(&pool, user, artist_id)
        .await
        .map_err(|x| {
            let err = format!("Error while getting releases: {x:?}");
            tracing::error!("{err}");
            ServerFnError::new("Could not retrieve releases, try again later")
        })?;
    Ok(ArtistsReleaseResult { releases })
}
