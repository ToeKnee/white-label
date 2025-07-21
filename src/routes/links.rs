//! Routes for managing music services for an artist.

use leptos::prelude::ServerFnError;
use leptos::server;
use server_fn::codec::Cbor;

use crate::models::music_service::MusicService;
use crate::models::social_media::SocialMediaService;
#[cfg(feature = "ssr")]
use crate::services::links::get_links_service;
#[cfg(feature = "ssr")]
use crate::state::pool;

/// Contains the result of fetching music services for an artist.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct LinksResult {
    /// A vector of music services associated with the artist.
    pub music_services: Vec<MusicService>,
    /// A vector of social links associated with the artist.
    pub social_media_services: Vec<SocialMediaService>,
}

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
#[server(GetMusicServices, "/api", endpoint="get_music_services", output = Cbor)]
pub async fn get_links(
    /// The slug of the artist.
    artist_slug: String,
) -> Result<LinksResult, ServerFnError> {
    let pool = pool()?;
    get_links_service(&pool, artist_slug).await
}
