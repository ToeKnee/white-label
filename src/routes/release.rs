use leptos::prelude::ServerFnError;
use leptos::server;
use server_fn::codec::Cbor;

use crate::forms::release::{CreateReleaseForm, UpdateReleaseForm};
#[cfg(feature = "ssr")]
use crate::models::record_label::RecordLabel;
use crate::models::{artist::Artist, release::Release, track_with_artists::TrackWithArtists};
#[cfg(feature = "ssr")]
use crate::services::release::{
    create_release_service, delete_release_service, get_next_scheduled_release_service,
    get_release_service, get_releases_service, update_release_service,
};
#[cfg(feature = "ssr")]
use crate::state::{auth, pool};

#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct ReleasesResult {
    pub releases: Vec<Release>,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct ReleaseResult {
    pub release: Release,
    pub artists: Vec<Artist>,
    pub tracks: Vec<TrackWithArtists>,
}

#[server(GetReleases, "/api", endpoint="get_releases", output = Cbor)]
pub async fn get_releases(artist_slug: String) -> Result<ReleasesResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    get_releases_service(&pool, user, artist_slug).await
}

#[server(GetNextScheduledRelease, "/api", endpoint="get_next_scheduled_release", output = Cbor)]
pub async fn get_next_scheduled_release(
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

#[server(GetRelease, "/api", endpoint="get_release", output = Cbor)]
pub async fn get_release(
    artist_slug: String,
    slug: String,
) -> Result<ReleaseResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    get_release_service(&pool, user, artist_slug, slug).await
}

#[server(CreateRelease, "/api", endpoint="create_release", output = Cbor)]
pub async fn create_release(form: CreateReleaseForm) -> Result<ReleaseResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    create_release_service(&pool, user, form).await
}

#[server(UpdateRelease, "/api", endpoint="update_release", output = Cbor)]
pub async fn update_release(form: UpdateReleaseForm) -> Result<ReleaseResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    update_release_service(&pool, user, form).await
}

#[server(DeleteRelease, "/api", endpoint="delete_release", output = Cbor)]
pub async fn delete_release(slug: String) -> Result<ReleaseResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    delete_release_service(&pool, user, slug).await
}
