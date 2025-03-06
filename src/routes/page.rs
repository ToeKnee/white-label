use leptos::prelude::ServerFnError;
use leptos::server;
use server_fn::codec::Cbor;

use crate::forms::page::{CreatePageForm, UpdatePageForm};
use crate::models::page::Page;

#[cfg(feature = "ssr")]
use crate::services::page::{
    create_page_service, delete_page_service, get_page_service, update_page_service,
};
#[cfg(feature = "ssr")]
use crate::state::{auth, pool};

#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct PageResult {
    pub page: Page,
}

#[server(GetPage, "/api", endpoint="get_page", output = Cbor)]
pub async fn get_page(slug: String) -> Result<PageResult, ServerFnError> {
    let pool = pool()?;
    get_page_service(&pool, slug).await
}

#[server(CreatePage, "/api", endpoint="create_page", output = Cbor)]
pub async fn create_page(page_form: CreatePageForm) -> Result<PageResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    create_page_service(&pool, user, page_form).await
}

#[server(UpdatePage, "/api", endpoint="update_page", output = Cbor)]
pub async fn update_page(page_form: UpdatePageForm) -> Result<PageResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    update_page_service(&pool, user, page_form).await
}

#[server(DeletePage, "/api", endpoint="delete_page", output = Cbor)]
pub async fn delete_page(slug: String) -> Result<PageResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    delete_page_service(&pool, user, slug).await
}
