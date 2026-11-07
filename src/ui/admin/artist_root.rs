//! The artist-tabs layout the artist admin editors render inside
//! (`Profile` / `Releases` / `Links` / `Images` above an `Outlet`).

use dioxus::prelude::*;

use crate::ui::Route;

/// Artist editor tabs layout.
///
/// `artist_slug` is injected by the router from the variant's field, which
/// is why the layout starts at the slug routes rather than at the nest base
/// ([`Route::CreateArtist`] has no slug to show tabs for). Active-tab state
/// is a plain comparison of the current route.
#[component]
pub fn AdminArtistRoot(artist_slug: String) -> Element {
    // Query codes carried by the profile, links and images editors (`?updated=1`,
    // `?bad=`, `?uploaded=1`, `?bad_upload=`) must not un-highlight their tabs,
    // so the comparison value drops them.
    let current = match use_route::<Route>() {
        Route::EditArtist {
            artist_slug: ref slug,
            ..
        } => Route::EditArtist {
            artist_slug: slug.clone(),
            updated: None,
            bad: None,
        },
        Route::EditArtistLinks {
            artist_slug: ref slug,
            ..
        } => Route::EditArtistLinks {
            artist_slug: slug.clone(),
            updated: None,
            bad: None,
        },
        Route::EditArtistImages {
            artist_slug: ref slug,
            ..
        } => Route::EditArtistImages {
            artist_slug: slug.clone(),
            uploaded: None,
            bad_upload: None,
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
        updated: None,
        bad: None,
    };
    let images = Route::EditArtistImages {
        artist_slug,
        uploaded: None,
        bad_upload: None,
    };

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
