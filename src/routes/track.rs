use leptos::prelude::ServerFnError;
use leptos::server;
use server_fn::codec::Cbor;

use crate::forms::track::{CreateTrackForm, UpdateTrackForm};
use crate::models::{artist::Artist, release::Release, track::Track};
#[cfg(feature = "ssr")]
use crate::services::track::{
    create_track_service, delete_track_service, get_track_service, get_tracks_service,
    update_track_service,
};
#[cfg(feature = "ssr")]
use crate::state::{auth, pool};

#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct TracksResult {
    pub tracks: Vec<Track>,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct TrackResult {
    pub track: Track,
    pub artists: Vec<Artist>,
    pub releases: Vec<Release>,
}

#[server(GetTracks, "/api", endpoint="get_tracks", output = Cbor)]
pub async fn get_tracks(
    artist_slug: String,
    release_slug: String,
) -> Result<TracksResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    get_tracks_service(&pool, user, artist_slug, release_slug).await
}

#[server(GetTrack, "/api", endpoint="get_track", output = Cbor)]
pub async fn get_track(
    artist_slug: String,
    release_slug: String,
    slug: String,
) -> Result<TrackResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    get_track_service(&pool, user, artist_slug, release_slug, slug).await
}

#[server(CreateTrack, "/api", endpoint="create_track", output = Cbor)]
pub async fn create_track(form: CreateTrackForm) -> Result<TrackResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    create_track_service(&pool, user, form).await
}

#[server(UpdateTrack, "/api", endpoint="update_track", output = Cbor)]
pub async fn update_track(form: UpdateTrackForm) -> Result<TrackResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    tracing::info!("Updating track with form: {:?}", form);
    update_track_service(&pool, user, form).await
}

#[server(DeleteTrack, "/api", endpoint="delete_track", output = Cbor)]
pub async fn delete_track(slug: String) -> Result<TrackResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    delete_track_service(&pool, user, slug).await
}
