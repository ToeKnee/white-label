//! The admin section: the `AdminRoot` layout (side menu + permission gate)
//! and its pages (MIGRATION.md Phase 4 slice 4), ported from the Leptos
//! `components::admin` group.
//!
//! The layout mirrors the Leptos `root.rs`: a flex row with the menu on the
//! left and the gated page body on the right. Differences from the original:
//!
//! - The gate goes through `use_user` (the `GET /api/user` twin) instead of
//!   the Leptos `UserContext`; the condition is the original's
//!   `permissions.contains("admin")` — guests and non-admins get the login
//!   page rendered inline (`ui::redirect` swaps the SSR content; dioxus has
//!   no 303 from inside SSR components — same deviation as slice 3).
//! - Menu links use the URLs the `admin_menu` service produces, as plain
//!   `href` anchors. The typed `Link` would fight the nest/slug URLs the
//!   service builds as strings; revisit once the client router lands.
//! - `Artists`/`Pages` summary links pointed at `/admin/artists` and
//!   `/admin/pages`, which the dioxus route table does not (and the Leptos
//!   one did not either, for `/admin/pages`) — they are plain text here.
//!   Slice 5 may add real index routes.
//! - Menu load failures fall back to the static skeleton (Dashboard link,
//!   empty submenus, create links), exactly where the Leptos component kept
//!   its `AdminMenu::default()`.

pub mod artist;
pub mod dashboard;
pub mod images;
pub mod label;
pub mod links;
pub mod pages;
pub mod release;
pub mod track;

// Re-exported so the route table can import them from `ui::admin`.
pub use artist::{CreateArtist, EditArtist};
pub use dashboard::Dashboard;
pub use images::EditArtistImages;
pub use label::EditLabel;
pub use links::EditArtistLinks;
pub use pages::{CreatePage, EditPage};
pub use release::{CreateRelease, EditRelease, Releases};
pub use track::{CreateTrack, EditTrack, TrackList, Tracks};

use dioxus::prelude::*;

use crate::api_types::menu::{AdminMenu as MenuData, MenuArtist, MenuPage, MenuRelease};
use crate::ui::Route;
use crate::ui::data::get_admin_menu;
use crate::ui::page_data;
use crate::ui::redirect;
use crate::ui::use_page;
use crate::ui::use_user;
use crate::ui::utils::Loading;

/// The admin layout: menu + permission gate around every admin page.
#[component]
pub fn AdminRoot() -> Element {
    rsx! {
        div { class: "flex",
            // The menu has its own boundary so a slow (or failing) admin
            // menu never blocks the page body, as the Leptos Transition did.
            SuspenseBoundary {
                fallback: |_context: SuspenseContext| rsx! { Loading {} },
                AdminMenuBody {}
            }
            SuspenseBoundary {
                fallback: |_context: SuspenseContext| rsx! { Loading {} },
                AdminGateBody {}
            }
        }
    }
}

/// The gate around the page body: signed-in users with the `admin`
/// permission get the child route; everyone else gets the login page.
#[component]
fn AdminGateBody() -> Element {
    let session = use_user()?;
    match session() {
        Ok(Some(user)) if user.permissions.contains("admin") => {}
        Ok(_) => {
            redirect(Route::Login { bad: None });
            return rsx! { Loading {} };
        }
        Err(error) => {
            tracing::error!("Error while getting session user for admin gate: {error}");
            redirect(Route::Login { bad: None });
            return rsx! { Loading {} };
        }
    }

    rsx! {
        article { class: "my-6 md:container md:mx-auto prose",
            Outlet::<Route> {}
        }
    }
}

/// The side menu, loaded from the `admin_menu` service (403/401 for
/// non-admins, which falls through to the skeleton below).
#[component]
fn AdminMenuBody() -> Element {
    let menu = use_page(move || page_data(get_admin_menu()))?;
    let menu = match menu() {
        Ok(menu) => menu,
        Err(error) => {
            tracing::error!("Error while getting admin menu: {error}");
            MenuData::default()
        }
    };

    rsx! {
        ul { class: "w-56 menu bg-base-200 rounded-box",
            li { a { href: "/admin", "Dashboard" } }
            li {
                a { href: menu.url,
                    "{menu.record_label.name}"
                }
            }
            li {
                ArtistsMenu { artists: menu.artists }
            }
            li {
                PagesMenu { pages: menu.pages }
            }
        }
    }
}

/// The artists submenu: one expandable row per artist plus the create
/// link (the Leptos `ArtistsMenu`).
#[component]
fn ArtistsMenu(artists: Vec<MenuArtist>) -> Element {
    rsx! {
        details { open: true,
            summary { "Artists" }
            ul {
                if artists.is_empty() {
                    li { "No artists yet..." }
                } else {
                    for menu_artist in artists {
                        li {
                            ArtistMenuRow { menu_artist }
                        }
                    }
                }
                li { a { href: "/admin/artist", "Create Artist" } }
            }
        }
    }
}

/// One artist row with its releases nested underneath (the Leptos
/// `ArtistMenuRow`; its "All Releases" and create links are built from the
/// artist slug exactly as the original did).
#[component]
fn ArtistMenuRow(menu_artist: MenuArtist) -> Element {
    let artist_slug = menu_artist.artist.slug.clone();
    let releases_url = format!("/admin/artist/{artist_slug}/releases");
    let create_release_url = format!("/admin/artist/{artist_slug}/releases/new");

    rsx! {
        details {
            summary {
                a { href: menu_artist.url,
                    "{menu_artist.artist.name}"
                }
            }
            ul {
                li { a { href: releases_url, "All Releases" } }
                if menu_artist.releases.is_empty() {
                    // The original linked the empty-state text to the
                    // create-release page; kept as-is.
                    li { a { href: create_release_url.clone(), "No releases yet..." } }
                } else {
                    for menu_release in menu_artist.releases {
                        li {
                            ReleaseMenuRow {
                                menu_release,
                                artist_id: menu_artist.artist.id,
                            }
                        }
                    }
                }
                li { a { href: create_release_url, "Create Release" } }
            }
        }
    }
}

/// The submenu icon decision: a release is "primary" for the artist whose
/// id matches its `primary_artist_id`, "featured" otherwise.
const fn artist_role(primary_artist_id: i64, artist_id: i64) -> (&'static str, &'static str) {
    if primary_artist_id == artist_id {
        ("Primary artist", "●")
    } else {
        ("Featured artist", "○")
    }
}

/// One release row: a filled dot marks the artist as the release's primary
/// artist, a hollow one means featured (the Leptos `ReleaseMenuRow`).
#[component]
fn ReleaseMenuRow(menu_release: MenuRelease, artist_id: i64) -> Element {
    let (icon_title, icon) = artist_role(menu_release.release.primary_artist_id, artist_id);
    let name = menu_release.release.name;

    rsx! {
        a { href: menu_release.url,
            span { title: icon_title, "{icon}" }
            " {name}"
        }
    }
}

/// The pages submenu: one row per page plus the create link (the Leptos
/// `PagesMenu`).
#[component]
fn PagesMenu(pages: Vec<MenuPage>) -> Element {
    rsx! {
        details {
            summary { "Pages" }
            ul {
                if pages.is_empty() {
                    li { "No pages yet..." }
                } else {
                    for menu_page in pages {
                        li {
                            a { href: menu_page.url,
                                "{menu_page.page.name}"
                            }
                        }
                    }
                }
                li { a { href: "/admin/page", "Create Page" } }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::artist_role;

    #[test]
    fn primary_and_featured_roles() {
        assert_eq!(artist_role(7, 7), ("Primary artist", "●"));
        assert_eq!(artist_role(3, 7), ("Featured artist", "○"));
    }
}
