//! Placeholder pages and admin layouts for the route table
//! (MIGRATION.md Phase 3).
//!
//! Every route variant needs a component by the end of Phase 3, so pages
//! that are not ported yet render these stubs. The Phase 4 slices delete
//! them as their real components land, per group:
//!
//! - public/auth pages → Phase 4 slices 2-3
//! - admin dashboard/label/pages → slice 4
//! - admin artist/release/track → slice 5

use dioxus::prelude::*;

use crate::ui::Route;

/// Page title, an optional "context" line (e.g. the slug from the URL), and
/// the migration marker shared by every placeholder.
fn page_body(title: &str, detail: Option<&str>) -> Element {
    rsx! {
        h1 { "{title}" }
        if let Some(detail) = detail {
            p { class: "text-base-content/60", "{detail}" }
        }
        p { class: "text-base-content/60",
            "Placeholder page — the real one arrives with the Phase 4 slices."
        }
    }
}

/// Standalone placeholder with Leptos' `article.prose` wrapper. Pages under
/// the admin tree skip the wrapper: the `AdminRoot` layout provides it.
fn prose_page(title: &str, detail: Option<&str>) -> Element {
    rsx! {
        article { class: "my-6 md:container md:mx-auto prose",
            {page_body(title, detail)}
        }
    }
}

/// Admin placeholder: no wrapper (the `AdminRoot` layout provides it).
fn admin_page(title: &str, detail: Option<&str>) -> Element {
    page_body(title, detail)
}

// ---------------------------------------------------------------------------
// Public site
// ---------------------------------------------------------------------------

/// Placeholder home page.
#[component]
pub fn RecordLabelHome() -> Element {
    prose_page("Record label home", None)
}

/// Placeholder artist index.
#[component]
pub fn ArtistsPage() -> Element {
    prose_page("Artists", None)
}

/// Placeholder artist page.
#[component]
pub fn ArtistPage(slug: String) -> Element {
    prose_page("Artist", Some(&format!("slug: {slug}")))
}

/// Placeholder release page.
#[component]
pub fn ReleasePage(artist_slug: String, release_slug: String) -> Element {
    prose_page(
        "Release",
        Some(&format!("artist: {artist_slug}, release: {release_slug}")),
    )
}

/// Placeholder marketing/legal page.
#[component]
pub fn PageDetails(slug: String) -> Element {
    prose_page("Page", Some(&format!("slug: {slug}")))
}

// ---------------------------------------------------------------------------
// Auth pages
// ---------------------------------------------------------------------------

/// Placeholder login page.
#[component]
pub fn Login() -> Element {
    prose_page("Log in", None)
}

/// Placeholder registration page.
#[component]
pub fn Register() -> Element {
    prose_page("Register", None)
}

/// Placeholder logout page (the Leptos original posts then redirects).
#[component]
pub fn Logout() -> Element {
    prose_page("Log out", None)
}

/// Placeholder profile editor.
#[component]
pub fn EditProfile() -> Element {
    prose_page("Edit profile", None)
}

/// Placeholder password change page.
#[component]
pub fn ChangePassword() -> Element {
    prose_page("Change password", None)
}

// ---------------------------------------------------------------------------
// Admin section
// ---------------------------------------------------------------------------

/// Admin chrome placeholder (Leptos: `components::admin::root::AdminRoot`).
///
/// Deliberately minimal until the Phase 4 slices: the admin side menu and
/// the "must have the admin permission, else redirect to /login" gate are
/// ported then. Until then every admin page below shows placeholder markup,
/// and the data endpoints underneath stay permission-checked server-side.
#[component]
pub fn AdminRoot() -> Element {
    rsx! {
        div { class: "flex",
            article { class: "my-6 md:container md:mx-auto prose",
                Outlet::<Route> {}
            }
        }
    }
}

/// Artist editor tabs layout (Leptos: `components::admin::artist::root`).
///
/// `artist_slug` is injected by the router from the variant's field, which
/// is why the layout starts at the slug routes rather than at the nest base
/// ([`Route::CreateArtist`] has no slug to show tabs for). Active-tab state
/// is a plain comparison of the current route — Phase 5 may refine it.
#[component]
pub fn AdminArtistRoot(artist_slug: String) -> Element {
    let current = use_route::<Route>();
    let profile = Route::EditArtist {
        artist_slug: artist_slug.clone(),
    };
    let releases = Route::Releases {
        artist_slug: artist_slug.clone(),
    };
    let links = Route::EditArtistLinks {
        artist_slug: artist_slug.clone(),
    };
    let images = Route::EditArtistImages { artist_slug };

    let tab_class = |tab: &Route| {
        if tab == &current {
            "tab tab-active"
        } else {
            "tab"
        }
    };

    rsx! {
        div { role: "tablist", class: "mb-6 tabs tabs-border not-prose",
            Link {
                to: profile.clone(),
                role: "tab",
                class: tab_class(&profile),
                "Profile"
            }
            Link {
                to: releases.clone(),
                role: "tab",
                class: tab_class(&releases),
                "Releases"
            }
            Link {
                to: links.clone(),
                role: "tab",
                class: tab_class(&links),
                "Links"
            }
            Link {
                to: images.clone(),
                role: "tab",
                class: tab_class(&images),
                "Images"
            }
        }
        Outlet::<Route> {}
    }
}

/// Placeholder dashboard.
#[component]
pub fn Dashboard() -> Element {
    admin_page("Admin dashboard", None)
}

/// Placeholder record-label editor.
#[component]
pub fn EditLabel() -> Element {
    admin_page("Edit label", None)
}

/// Placeholder artist creation form.
#[component]
pub fn CreateArtist() -> Element {
    admin_page("Create artist", None)
}

/// Placeholder artist editor.
#[component]
pub fn EditArtist(artist_slug: String) -> Element {
    admin_page("Edit artist", Some(&format!("slug: {artist_slug}")))
}

/// Placeholder artist links editor.
#[component]
pub fn EditArtistLinks(artist_slug: String) -> Element {
    admin_page("Edit artist links", Some(&format!("slug: {artist_slug}")))
}

/// Placeholder artist images editor.
#[component]
pub fn EditArtistImages(artist_slug: String) -> Element {
    admin_page("Edit artist images", Some(&format!("slug: {artist_slug}")))
}

/// Placeholder releases list.
#[component]
pub fn Releases(artist_slug: String) -> Element {
    admin_page("Releases", Some(&format!("artist: {artist_slug}")))
}

/// Placeholder release creation form.
#[component]
pub fn CreateRelease(artist_slug: String) -> Element {
    admin_page("Create release", Some(&format!("artist: {artist_slug}")))
}

/// Placeholder release editor.
#[component]
pub fn EditRelease(artist_slug: String, release_slug: String) -> Element {
    admin_page(
        "Edit release",
        Some(&format!("artist: {artist_slug}, release: {release_slug}")),
    )
}

/// Placeholder tracks list.
#[component]
pub fn Tracks(artist_slug: String, release_slug: String) -> Element {
    admin_page(
        "Tracks",
        Some(&format!("artist: {artist_slug}, release: {release_slug}")),
    )
}

/// Placeholder track creation form.
#[component]
pub fn CreateTrack(artist_slug: String, release_slug: String) -> Element {
    admin_page(
        "Create track",
        Some(&format!("artist: {artist_slug}, release: {release_slug}")),
    )
}

/// Placeholder track editor.
#[component]
pub fn EditTrack(artist_slug: String, release_slug: String, track_slug: String) -> Element {
    admin_page(
        "Edit track",
        Some(&format!(
            "artist: {artist_slug}, release: {release_slug}, track: {track_slug}"
        )),
    )
}

/// Placeholder page creation form.
#[component]
pub fn CreatePage() -> Element {
    admin_page("Create page", None)
}

/// Placeholder page editor.
#[component]
pub fn EditPage(slug: String) -> Element {
    admin_page("Edit page", Some(&format!("slug: {slug}")))
}
