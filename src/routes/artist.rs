use leptos::prelude::ServerFnError;
use leptos::server;

use crate::models::artist::Artist;
#[cfg(feature = "ssr")]
use crate::state::pool;

#[derive(serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct ArtistResult {
    pub artist: Artist,
}

#[server]
pub async fn get_artist(slug: String) -> Result<ArtistResult, ServerFnError> {
    let pool = pool()?;
    Ok(ArtistResult {
        artist: Artist::get_by_slug(&pool, slug).await.map_err(|x| {
            let err = format!("Error while getting artist: {x:?}");
            tracing::error!("{err}");
            ServerFnError::new("Could not retrieve artist, try again later")
        })?,
    })
}
