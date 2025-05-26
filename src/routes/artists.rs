use leptos::prelude::ServerFnError;
use leptos::server;
use server_fn::codec::Cbor;

use crate::models::release::Release;
#[cfg(feature = "ssr")]
use crate::services::artists::get_releases_for_artists_service;
#[cfg(feature = "ssr")]
use crate::state::{auth, pool};

#[derive(serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct ArtistsReleaseResult {
    pub releases: Vec<Release>,
}

#[server(GetLabelReleases, "/api", endpoint="record_label_releases", output = Cbor)]
pub async fn get_releases_for_artists(artist_ids: String) -> Result<ArtistsReleaseResult, ServerFnError> {
    let auth = auth()?;
    let pool = pool()?;
    let user = auth.current_user.as_ref();

    let artist_ids = artist_ids.split(',').map(|s| s.parse::<i64>().unwrap_or_default()).collect::<Vec<_>>();

    let releases = get_releases_for_artists_service(&pool, user, artist_ids).await.map_err(|x| {
        let err = format!("Error while getting releases: {x:?}");
        tracing::error!("{err}");
        ServerFnError::new("Could not retrieve releases, try again later")
    })?;
    Ok(ArtistsReleaseResult { releases })
}
