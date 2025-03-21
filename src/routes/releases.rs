use leptos::prelude::ServerFnError;
use leptos::server;
use server_fn::codec::Cbor;

use crate::models::release::Release;

#[cfg(feature = "ssr")]
use crate::services::releases::get_releases_service;
#[cfg(feature = "ssr")]
use crate::state::{auth, pool};

#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct ReleasesResult {
    pub releases: Vec<Release>,
}

#[server(GetReleases, "/api", endpoint="get_releases", output = Cbor)]
pub async fn get_releases(slug: String) -> Result<ReleasesResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    get_releases_service(&pool, user, slug).await
}
