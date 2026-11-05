//! Routes for handling admin menu data.
use leptos::prelude::ServerFnError;
use leptos::server;

#[cfg(feature = "ssr")]
use crate::services::menu::admin_menu;
#[cfg(feature = "ssr")]
use crate::state::{auth, pool};

pub use crate::api_types::menu::{AdminMenu, MenuArtist, MenuPage, MenuRelease};

/// Get data for the admin menu.
///
/// # Returns:
/// A `AdminMenu` containing the data for the admin menu.
///
/// # Errors:
/// Will return a `ServerFnError` if there is an issue retrieving the record label.
#[server(GetRecordLabel, "/api", endpoint = "admin_menu")]
pub async fn get_admin_menu() -> Result<AdminMenu, ServerFnError> {
    let pool = pool()?;
    let auth = auth().await?;

    let user = auth.current_user.as_ref();

    admin_menu(&pool, user).await.map_err(ServerFnError::from)
}
