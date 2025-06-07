//! Routes for `Page` management.
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

/// Contains a single Page.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct PageResult {
    /// The page being fetched or modified.
    pub page: Page,
}

/// Get a specific page by its slug.
///
/// # Arguments:
/// * `slug`: The slug of the page to fetch.
///
/// # Returns:
/// * A `PageResult` containing the page associated with the specified slug.
///
/// # Errors:
/// Will return a `ServerFnError` if the page cannot be found, or if there is an issue with the database connection.
#[server(GetPage, "/api", endpoint="get_page", output = Cbor)]
pub async fn get_page(
    /// The slug of the page to fetch.
    slug: String,
) -> Result<PageResult, ServerFnError> {
    let pool = pool()?;
    get_page_service(&pool, slug).await
}

/// Create a new page.
///
/// # Arguments:
/// * `page_form`: The form data for creating a new page.
///
/// # Returns:
/// * A `PageResult` containing the newly created page.
///
/// # Errors:
/// Will return a `ServerFnError` if there is an issue with the database connection or if the user is not authenticated.
#[server(CreatePage, "/api", endpoint="create_page", output = Cbor)]
pub async fn create_page(
    /// The form data for creating a new page.
    page_form: CreatePageForm,
) -> Result<PageResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    create_page_service(&pool, user, page_form).await
}

/// Update an existing page.
///
/// # Arguments:
/// * `page_form`: The form data for updating an existing page.
///
/// # Returns:
/// * A `PageResult` containing the updated page.
///
/// # Errors:
/// Will return a `ServerFnError` if there is an issue with the database connection or if the user is not authenticated.
#[server(UpdatePage, "/api", endpoint="update_page", output = Cbor)]
pub async fn update_page(
    /// The form data for updating an existing page.
    page_form: UpdatePageForm,
) -> Result<PageResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    update_page_service(&pool, user, page_form).await
}

/// Delete a page by its slug.
///
/// # Arguments:
/// * `slug`: The slug of the page to delete.
///
/// # Returns:
/// * A `PageResult` containing the deleted page.
///
/// # Errors:
/// Will return a `ServerFnError` if there is an issue with the database connection or if the user is not authenticated.
#[server(DeletePage, "/api", endpoint="delete_page", output = Cbor)]
pub async fn delete_page(
    /// The slug of the page to delete.
    slug: String,
) -> Result<PageResult, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();
    delete_page_service(&pool, user, slug).await
}
