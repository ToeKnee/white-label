//! Routes for handling admin menu data.
use leptos::prelude::ServerFnError;
use leptos::server;
use server_fn::codec::Cbor;

use crate::models::{artist::Artist, page::Page, record_label::RecordLabel, release::Release};
#[cfg(feature = "ssr")]
use crate::services::menu::admin_menu;
#[cfg(feature = "ssr")]
use crate::state::{auth, pool};

/// `AdminMenu` struct contains nested details to generate an admin menu.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct AdminMenu {
    /// The record label associated with the admin menu.
    pub record_label: RecordLabel,
    /// The URL for editing the record label.
    pub url: String,
    /// A vector of artists associated with the record label.
    pub artists: Vec<MenuArtist>,
    /// A vector of pages associated with the record label.
    pub pages: Vec<MenuPage>,
}

/// `MenuArtist` struct contains details about an artist and their releases for the admin menu.
/// This struct is used to generate a sub-menu for each artist in the admin menu.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct MenuArtist {
    /// The artist associated with the sub-menu.
    pub artist: Artist,
    /// The URL for editing the artist.
    pub url: String,
    /// A vector of releases associated with the artist.
    pub releases: Vec<MenuRelease>,
}

/// `MenuRelease` struct contains details about a release and its tracks for the admin menu.
/// This struct is used to generate a sub-menu for each release in the artist's sub-menu.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct MenuRelease {
    /// The release associated with the sub-menu.
    pub release: Release,
    /// The URL for editing the release.
    pub url: String,
}

/// `MenuPage` struct contains details about a page for the admin menu.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct MenuPage {
    /// The page associated with the menu.
    pub page: Page,
    /// The URL for editing the page.
    pub url: String,
}

/// Get data for the admin menu.
///
/// # Returns:
/// A `AdminMenu` containing the data for the admin menu.
///
/// # Errors:
/// Will return a `ServerFnError` if there is an issue retrieving the record label.
#[server(GetRecordLabel, "/api", endpoint="admin_menu", output = Cbor)]
pub async fn get_admin_menu() -> Result<AdminMenu, ServerFnError> {
    let auth = auth()?;
    let pool = pool()?;
    let user = auth.current_user.as_ref();

    admin_menu(&pool, user).await
}
