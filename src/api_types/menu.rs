//! Response types for the admin menu.

use crate::models::{artist::Artist, page::Page, record_label::RecordLabel, release::Release};

/// `AdminMenu` struct contains nested details to generate an admin menu.
/// `PartialEq` for the Dioxus admin menu components (`ui::admin`).
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug, Eq, PartialEq)]
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
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug, Eq, PartialEq)]
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
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug, Eq, PartialEq)]
pub struct MenuRelease {
    /// The release associated with the sub-menu.
    pub release: Release,
    /// The URL for editing the release.
    pub url: String,
}

/// `MenuPage` struct contains details about a page for the admin menu.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug, Eq, PartialEq)]
pub struct MenuPage {
    /// The page associated with the menu.
    pub page: Page,
    /// The URL for editing the page.
    pub url: String,
}
