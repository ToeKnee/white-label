use leptos::prelude::ServerFnError;
use leptos::server;

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
    get_artist_service(pool, slug).await
}

#[server]
pub async fn create_artist(
    name: String,
    description: String,
    record_label_id: i64,
) -> Result<ArtistResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    create_artist_service(pool, user, name, description, record_label_id).await
}

#[server]
pub async fn update_artist(
    slug: String,
    name: String,
    description: String,
) -> Result<ArtistResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    update_artist_service(pool, user, slug, name, description).await
}

#[server]
pub async fn delete_artist(slug: String) -> Result<ArtistResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    delete_artist_service(pool, user, slug).await
}
