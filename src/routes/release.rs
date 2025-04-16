use leptos::prelude::ServerFnError;
use leptos::server;
use server_fn::codec::Cbor;

use crate::forms::release::CreateReleaseForm;
use crate::models::{artist::Artist, release::Release};
#[cfg(feature = "ssr")]
use crate::services::release::{create_release_service, get_releases_service};
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
}

#[server(GetReleases, "/api", endpoint="get_releases", output = Cbor)]
pub async fn get_releases(slug: String) -> Result<ReleasesResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    get_releases_service(&pool, user, slug).await
}

#[server(CreateRelease, "/api", endpoint="create_release", output = Cbor)]
pub async fn create_release(
    release_form: CreateReleaseForm,
) -> Result<ReleaseResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    create_release_service(&pool, user, release_form).await
}
