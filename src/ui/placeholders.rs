//! Placeholder pages and the artist-tab layout for the route table
//! (MIGRATION.md Phase 3).
//!
//! Every route variant needs a component by the end of Phase 3, so pages
//! that are not ported yet render these stubs. The Phase 4 slices delete
//! them as their real components land, per group:
//!
//! - public pages → done (Phase 4 slice 2: see `record_label`, `artist`,
//!   `release` and `page`)
//! - auth pages → done (Phase 4 slice 3: see `auth`)
//! - admin dashboard/label/pages and the `AdminRoot` layout → done
//!   (Phase 4 slice 4: see `admin`)
//! - admin artist profile (`CreateArtist`/`EditArtist`) → done
//!   (Phase 4 slice 5a: see `admin::artist`)
//! - admin releases list/forms (`Releases`/`CreateRelease`/`EditRelease`)
//!   → done (Phase 4 slice 5b: see `admin::release`)
//! - admin artist links/images and tracks → slice 5

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

/// Admin placeholder: no wrapper (the `AdminRoot` layout provides it).
fn admin_page(title: &str, detail: Option<&str>) -> Element {
    page_body(title, detail)
}

// ---------------------------------------------------------------------------
// Admin section (the `AdminRoot` layout is real since slice 4: `ui::admin`)
// ---------------------------------------------------------------------------

/// Artist editor tabs layout (Leptos: `components::admin::artist::root`).
///
/// `artist_slug` is injected by the router from the variant's field, which
/// is why the layout starts at the slug routes rather than at the nest base
/// ([`Route::CreateArtist`] has no slug to show tabs for). Active-tab state
/// is a plain comparison of the current route — Phase 5 may refine it.
#[component]
pub fn AdminArtistRoot(artist_slug: String) -> Element {
    // Query codes carried by the profile editor (`?updated=1`, `?bad=`)
    // must not un-highlight its tab, so the comparison value drops them.
    let current = match use_route::<Route>() {
        Route::EditArtist {
            artist_slug: ref slug,
            ..
        } => Route::EditArtist {
            artist_slug: slug.clone(),
            updated: None,
            bad: None,
        },
        other => other,
    };
    let profile = Route::EditArtist {
        artist_slug: artist_slug.clone(),
        updated: None,
        bad: None,
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
