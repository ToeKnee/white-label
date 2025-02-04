use leptos::prelude::ServerFnError;
use leptos::server;

use crate::forms::artist::{CreateArtistForm, UpdateArtistForm};
use crate::models::artist::Artist;

#[cfg(feature = "ssr")]
use crate::services::artist::{
    create_artist_service, delete_artist_service, get_artist_service, update_artist_service,
};
#[cfg(feature = "ssr")]
use crate::state::{auth, pool};

#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct ArtistResult {
    pub artist: Artist,
}

#[server]
pub async fn get_artist(slug: String) -> Result<ArtistResult, ServerFnError> {
    let pool = pool()?;
    get_artist_service(&pool, slug).await
}

#[server]
pub async fn create_artist(artist_form: CreateArtistForm) -> Result<ArtistResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    create_artist_service(&pool, user, artist_form).await
}

#[server]
pub async fn update_artist(artist_form: UpdateArtistForm) -> Result<ArtistResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    update_artist_service(&pool, user, artist_form).await
}

#[server]
pub async fn delete_artist(slug: String) -> Result<ArtistResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    delete_artist_service(&pool, user, slug).await
}
