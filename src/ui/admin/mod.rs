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
//!   empty submenus, create links), the shape `AdminMenu::default()`
//!   yields.
//! - Deleted rows are hidden by default; the "Show deleted" checkbox at
//!   the top of the menu flips the [`crate::ui::admin::AdminPrefs`] signal,
//!   which re-runs the menu load and every admin listing that read it (they
//!   read it inside their `use_page` closure, the one place
//!   [`crate::ui::use_page`] tracks). See [`crate::models::Visibility`] for
//!   the ladder behind it.

pub mod artist;
pub mod artist_root;
pub mod dashboard;
pub mod images;
pub mod label;
pub mod links;
pub mod pages;
pub mod release;
pub mod stats;
pub mod track;

// Re-exported so the route table can import them from `ui::admin`.
pub use artist::{CreateArtist, EditArtist};
pub use dashboard::Dashboard;
pub use images::EditArtistImages;
pub use label::EditLabel;
pub use links::EditArtistLinks;
pub use pages::{CreatePage, EditPage};
pub use release::{CreateRelease, EditRelease, Releases};
pub use stats::{ArtistStats, LabelStats, ReleaseStats};
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

/// The admin section's shared preferences, provided by `AdminRoot` and
/// read by the menu and the listing pages.
#[derive(Clone, Copy)]
pub struct AdminPrefs {
    /// The "Show deleted" toggle: when on, admin listings reach the
    /// `Deleted` rung of [`crate::models::Visibility`] and soft-deleted
    /// rows appear. Default off: deleted rows are hidden.
    pub show_deleted: Signal<bool>,
    /// Counts the reactive saves made anywhere in the admin section. Read
    /// inside the side menu's page load (in `AdminMenuBody`) and bumped by
    /// the editors through `AdminPrefs::refresh_menu`, so the menu follows
    /// creates, renames, soft-deletes and restores that never reload the
    /// document.
    pub menu_version: Signal<u64>,
}

impl AdminPrefs {
    /// Tell the side menu its listing is stale.
    ///
    /// Like every [`crate::ui::use_page`] load, the menu's resolves once per
    /// mount, and `AdminRoot` is a layout that outlives every admin
    /// navigation — so without this nudge a reactive save would leave the
    /// menu's rows (and the slug behind each artist/release link) frozen
    /// until the next document load. Editors call it from inside their
    /// `use_action` future when a save is accepted. It is deliberately *not*
    /// called from a `use_effect`: reading the counter to bump it there would
    /// subscribe the effect to its own write and loop.
    pub(crate) fn refresh_menu(mut self) {
        self.menu_version.with_mut(|version| *version += 1);
    }
}

/// The admin layout: menu + permission gate around every admin page.
#[component]
pub fn AdminRoot() -> Element {
    // Provided here (above the Outlet) so every admin page and the menu
    // share one toggle that survives admin navigation. The signal is made
    // outside the provider closure — hooks inside a provider closure
    // violate Dioxus's hook rules and panic during render.
    let show_deleted = use_signal(|| false);
    let menu_version = use_signal(|| 0);
    use_context_provider(|| AdminPrefs {
        show_deleted,
        menu_version,
    });

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
/// non-admins, which falls through to the skeleton below). The
/// "Show deleted" checkbox flips the shared signal; reading it inside the
/// `use_page` closure re-runs the menu load when it changes — as does
/// reading the save counter that [`AdminPrefs::refresh_menu`] bumps.
#[component]
fn AdminMenuBody() -> Element {
    let prefs = use_context::<AdminPrefs>();
    let mut show_deleted = prefs.show_deleted;
    let menu_version = prefs.menu_version;
    let menu = use_page(move || {
        // Reading the counter here is what makes a reactive save re-run this
        // load; see [`AdminPrefs::refresh_menu`].
        let _menu_version = menu_version();
        page_data(get_admin_menu(show_deleted()))
    })?;
    let menu = match menu() {
        Ok(menu) => menu,
        Err(error) => {
            tracing::error!("Error while getting admin menu: {error}");
            MenuData::default()
        }
    };

    rsx! {
        ul { class: "w-56 menu bg-base-200 rounded-box",
            li {
                // Without javascript this checkbox does nothing, so the
                // no-JS admin surface keeps deleted rows hidden
                // (restore still works from a row's own editor).
                label { class: "flex items-center gap-2 cursor-pointer",
                    input {
                        r#type: "checkbox",
                        checked: show_deleted(),
                        onchange: move |event| show_deleted.set(event.checked()),
                    }
                    "Show deleted"
                }
            }
            li { Link { to: Route::Dashboard { ready: None, purged: None, skipped: None, bad: None, files: None }, "Dashboard" } }
            li {
                Link { to: Route::EditLabel { updated: None, bad: None },
                    "{menu.record_label.name}"
                }
                // The label's play statistics ride along with the label. The
                // menu only appears for `admin`, while the stats page is
                // `label_owner` gated: an admin without that permission gets
                // steered to the dashboard by the page itself.
                Link {
                    to: Route::LabelStats {
                        range: None,
                        sort: None,
                    },
                    class: "text-sm",
                    "Play statistics"
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
            (
                Route::Dashboard {
                    ready: None,
                    purged: None,
                    skipped: None,
                    bad: None,
                    files: None,
                },
                "/admin?",
            ),
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
