//! The admin section: the `AdminRoot` layout (side menu + permission gate)
//! and its pages.
//!
//! The layout is a flex row with the menu on the left and the gated page
//! body on the right. Behaviour notes:
//!
//! - The gate goes through `use_user` (the `GET /api/user` server fn) and
//!   checks `permissions.contains("admin")` — guests and non-admins get the
//!   login page rendered inline (`ui::redirect` swaps the SSR content; dioxus
//!   has no 303 from inside SSR components).
//! - Menu links are typed `Link`s built from the menu data's slugs (the
//!   route table's own paths), not the `url` strings the `admin_menu`
//!   service builds: on a desktop window the document origin is `file://`,
//!   so a plain `href` anchor resolves outside the app and the window hands
//!   it to the system browser, while a typed `Link` navigates inside it.
//!   The `url` strings the `admin_menu` service builds now have no
//!   consumers.
//! - `Artists`/`Pages` summary links are plain text, because the route
//!   table has no `/admin/artists` or `/admin/pages` index routes to point
//!   them at.
//! - Menu load failures fall back to the static skeleton (Dashboard link,
//!   empty submenus, create links), the shape `AdminMenu::default()` yields.

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
use dioxus::router::Link;

/// The admin layout: menu + permission gate around every admin page.
#[component]
pub fn AdminRoot() -> Element {
    rsx! {
        div { class: "flex",
            // The menu has its own boundary so a slow (or failing) admin
            // menu never blocks the page body.
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
            li { Link { to: Route::Dashboard {}, "Dashboard" } }
            li {
                Link { to: Route::EditLabel { updated: None, bad: None },
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
/// link.
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
                li { Link { to: Route::CreateArtist { bad: None }, "Create Artist" } }
            }
        }
    }
}

/// One artist row with its releases nested underneath. The "All Releases"
/// and create links are built from the artist slug.
#[component]
fn ArtistMenuRow(menu_artist: MenuArtist) -> Element {
    let artist_slug = menu_artist.artist.slug.clone();
    let artist_id = menu_artist.artist.id;

    rsx! {
        details {
            summary {
                Link {
                    to: Route::EditArtist {
                        artist_slug: menu_artist.artist.slug,
                        updated: None,
                        bad: None,
                    },
                    "{menu_artist.artist.name}"
                }
            }
            ul {
                li {
                    Link { to: Route::Releases { artist_slug: artist_slug.clone() }, "All Releases" }
                }
                if menu_artist.releases.is_empty() {
                    // The original linked the empty-state text to the
                    // create-release page; kept as-is.
                    li {
                        Link {
                            to: Route::CreateRelease {
                                artist_slug: artist_slug.clone(),
                                bad: None,
                            },
                            "No releases yet..."
                        }
                    }
                } else {
                    for menu_release in menu_artist.releases {
                        li {
                            ReleaseMenuRow {
                                menu_release,
                                artist_id,
                                artist_slug: artist_slug.clone(),
                            }
                        }
                    }
                }
                li {
                    Link {
                        to: Route::CreateRelease { artist_slug, bad: None },
                        "Create Release"
                    }
                }
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
/// artist, a hollow one means featured. The
/// artist slug comes from the row above: the route table nests a release
/// under its artist, and the menu's `url` string is no longer what the
/// link points at.
#[component]
fn ReleaseMenuRow(menu_release: MenuRelease, artist_id: i64, artist_slug: String) -> Element {
    let (icon_title, icon) = artist_role(menu_release.release.primary_artist_id, artist_id);
    let name = menu_release.release.name;

    rsx! {
        Link {
            to: Route::EditRelease {
                artist_slug,
                release_slug: menu_release.release.slug,
                updated: None,
                bad: None,
                uploaded: None,
                bad_upload: None,
            },
            span { title: icon_title, "{icon}" }
            " {name}"
        }
    }
}

/// The pages submenu: one row per page plus the create link.
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
                            Link {
                                to: Route::EditPage {
                                    slug: menu_page.page.slug,
                                    updated: None,
                                    bad: None,
                                },
                                "{menu_page.page.name}"
                            }
                        }
                    }
                }
                li { Link { to: Route::CreatePage { bad: None }, "Create Page" } }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_and_featured_roles() {
        assert_eq!(artist_role(7, 7), ("Primary artist", "●"));
        assert_eq!(artist_role(3, 7), ("Featured artist", "○"));
    }

    /// The menu's typed targets address the same pages the `admin_menu`
    /// service names with its `url` strings.
    ///
    /// Dioxus renders a canonical href for an optional-query route: a bare
    /// `?` when the query is empty, and a nest-base path keeps its trailing
    /// slash (so the artist editor is `…/jimi/?`). Those hrefs parse back to
    /// the same route, and the e2e pins allow the optional `?`/`/?`, so the
    /// menu now links the canonical forms — a deliberate change from the
    /// service's clean strings.
    #[test]
    fn menu_routes_address_the_pages_the_service_names() {
        let cases = [
            (Route::Dashboard {}, "/admin"),
            (
                Route::EditLabel {
                    updated: None,
                    bad: None,
                },
                "/admin/label?",
            ),
            (Route::CreateArtist { bad: None }, "/admin/artist?"),
            (Route::CreatePage { bad: None }, "/admin/page?"),
            (
                Route::EditArtist {
                    artist_slug: "jimi".to_string(),
                    updated: None,
                    bad: None,
                },
                "/admin/artist/jimi/?",
            ),
            (
                Route::Releases {
                    artist_slug: "jimi".to_string(),
                },
                "/admin/artist/jimi/releases",
            ),
            (
                Route::CreateRelease {
                    artist_slug: "jimi".to_string(),
                    bad: None,
                },
                "/admin/artist/jimi/releases/new?",
            ),
            (
                Route::EditRelease {
                    artist_slug: "jimi".to_string(),
                    release_slug: "axis".to_string(),
                    updated: None,
                    bad: None,
                    uploaded: None,
                    bad_upload: None,
                },
                "/admin/artist/jimi/release/axis?",
            ),
            (
                Route::EditPage {
                    slug: "social-tips".to_string(),
                    updated: None,
                    bad: None,
                },
                "/admin/page/social-tips?",
            ),
        ];
        for (route, canonical) in cases {
            assert_eq!(route.to_string(), canonical, "canonical href for {route:?}");
            let href: Route = canonical.parse().expect("canonical menu href parses back");
            assert_eq!(href, route, "{canonical} should parse back to {route:?}");
        }
    }
}
