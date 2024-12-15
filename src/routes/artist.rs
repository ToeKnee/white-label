use leptos::prelude::ServerFnError;
use leptos::server;
// use leptos_router::*;

use crate::models::artist::Artist;

#[derive(serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct ArtistResult {
    pub artist: Artist,
}

#[server(GetArtist, "/api", "GetJson")]
pub async fn get_artist(slug: String) -> Result<ArtistResult, ServerFnError> {
    Ok(ArtistResult {
        artist: Artist::get_by_slug(slug).await.map_err(|x| {
            let err = format!("Error while getting artist: {x:?}");
            tracing::error!("{err}");
            ServerFnError::new("Could not retrieve artist, try again later")
        })?,
    })
}
