//! The application route table (mirrors the Leptos route tree in `crate::app`,
//! see MIGRATION.md Phase 3).
//!
//! Layouts are the dioxus equivalent of Leptos `ParentRoute`: they wrap the
//! variants below them (until `#[end_layout]`) and render their child through
//! `Outlet::<Route>`.
//!
//! Example — `/admin/artist/jimi/release/axis/tracks` renders `AppShell` >
//! `AdminRoot` > `AdminArtistRoot` > `Tracks`.
//!
//! Leptos paired every `path!("x")` route with an exact-`"x/"` twin; the
//! dioxus parser matches both forms for one variant, so those duplicates
//! collapse (verified below and by the SSR tests in `crate::server`).

use dioxus::prelude::*;

use super::artist::{ArtistPage, ArtistsPage};
use super::page::PageDetails;
use super::placeholders::{
    AdminArtistRoot, AdminRoot, ChangePassword, CreateArtist, CreatePage, CreateRelease,
    CreateTrack, Dashboard, EditArtist, EditArtistImages, EditArtistLinks, EditLabel, EditPage,
    EditProfile, EditRelease, EditTrack, Login, Logout, Register, Releases, Tracks,
};
use super::record_label::RecordLabelHome;
use super::release::ReleasePage;
use super::shell::AppShell;
use super::utils::NotFound;

/// Every page in the application.
///
/// Variant fields (slugs) become component props; the canonical URL for a
/// route sitting at its nest base ends with a trailing slash
/// (`/admin/artist/jimi/` for [`Route::EditArtist`]), but parsing accepts
/// both forms.
#[derive(Clone, Debug, PartialEq, Eq, Routable)]
pub enum Route {
    /// Header/main/footer chrome wrapping every route below it.
    #[layout(AppShell)]
    /// Landing page for the record label.
    #[route("/")]
    RecordLabelHome {},
    /// Index of the label's artists.
    #[route("/artists")]
    ArtistsPage {},
    /// One artist's public page.
    #[route("/artists/:slug")]
    ArtistPage {
        /// The artist slug.
        slug: String,
    },
    /// One release, addressed through its artist.
    #[route("/artists/:artist_slug/:release_slug")]
    ReleasePage {
        /// The artist slug.
        artist_slug: String,
        /// The release slug.
        release_slug: String,
    },
    /// A marketing/legal page by slug.
    #[route("/pages/:slug")]
    PageDetails {
        /// The page slug.
        slug: String,
    },
    /// Log in (session cookie).
    #[route("/login")]
    Login {},
    /// Create an account.
    #[route("/register")]
    Register {},
    /// Destroy the session.
    #[route("/logout")]
    Logout {},
    /// Edit the signed-in user's profile.
    #[route("/profile")]
    EditProfile {},
    /// Change the signed-in user's password.
    #[route("/profile/change-password")]
    ChangePassword {},
    /// Admin section: menu + permission gate live in the layout.
    #[layout(AdminRoot)]
    /// Admin dashboard.
    #[route("/admin")]
    Dashboard {},
    /// Edit the record label's details.
    #[route("/admin/label")]
    EditLabel {},
    /// Artist management under `/admin/artist`. The slug sits in the nest
    /// path, so every variant inside carries it as a prop — the layout
    /// receives it too (which is how `AdminArtistRoot` renders its tabs).
    /// `CreateArtist` lives outside the nest: it has no slug to carry.
    #[route("/admin/artist")]
    CreateArtist {},
    #[nest("/admin/artist/:artist_slug")]
    /// Artist editor tabs (profile/releases/links/images) wrapping the
    /// per-artist routes below.
    #[layout(AdminArtistRoot)]
    /// Edit an artist's details. Canonical URL ends with a slash
    /// (`/admin/artist/jimi/`); the form without one still parses.
    #[route("/")]
    EditArtist {
        /// The artist slug.
        artist_slug: String,
    },
    /// Edit an artist's external links.
    #[route("/links")]
    EditArtistLinks {
        /// The artist slug.
        artist_slug: String,
    },
    /// Upload/replace an artist's images.
    #[route("/images")]
    EditArtistImages {
        /// The artist slug.
        artist_slug: String,
    },
    /// List an artist's releases.
    #[route("/releases")]
    Releases {
        /// The artist slug.
        artist_slug: String,
    },
    /// Create a release for an artist.
    #[route("/releases/new")]
    CreateRelease {
        /// The artist slug.
        artist_slug: String,
    },
    /// Edit a release.
    #[route("/release/:release_slug")]
    EditRelease {
        /// The artist slug.
        artist_slug: String,
        /// The release slug.
        release_slug: String,
    },
    /// List a release's tracks.
    #[route("/release/:release_slug/tracks")]
    Tracks {
        /// The artist slug.
        artist_slug: String,
        /// The release slug.
        release_slug: String,
    },
    /// Create a track on a release.
    #[route("/release/:release_slug/tracks/new")]
    CreateTrack {
        /// The artist slug.
        artist_slug: String,
        /// The release slug.
        release_slug: String,
    },
    /// Edit a track.
    #[route("/release/:release_slug/track/:track_slug")]
    EditTrack {
        /// The artist slug.
        artist_slug: String,
        /// The release slug.
        release_slug: String,
        /// The track slug.
        track_slug: String,
    },
    #[end_layout]
    #[end_nest]
    /// Create a marketing/legal page.
    #[route("/admin/page")]
    CreatePage {},
    /// Edit a marketing/legal page.
    #[route("/admin/page/:slug")]
    EditPage {
        /// The page slug.
        slug: String,
    },
    #[end_layout]
    /// Catch-all 404 page (the Leptos `Routes fallback`).
    #[route("/:..segments")]
    NotFound {
        /// The unmatched path, split into segments.
        segments: Vec<String>,
    },
}

/// Navigates to `to` without a full page load, replacing the current entry
/// (the Leptos `redirect` helper's equivalent).
///
/// Call it from component code — it looks the navigator up in the current
/// scope:
///
/// ```ignore
/// redirect(Route::Login {});
/// ```
pub fn redirect(to: Route) {
    use_navigator().replace(to);
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::Route;

    fn parse(path: &str) -> Route {
        Route::from_str(path).unwrap_or_else(|error| panic!("`{path}` should parse: {error}"))
    }

    #[test]
    fn public_routes_parse() {
        assert_eq!(parse("/"), Route::RecordLabelHome {});
        assert_eq!(parse("/artists"), Route::ArtistsPage {});
        assert_eq!(
            parse("/artists/jimi"),
            Route::ArtistPage {
                slug: "jimi".into()
            }
        );
        assert_eq!(
            parse("/artists/jimi/axis"),
            Route::ReleasePage {
                artist_slug: "jimi".into(),
                release_slug: "axis".into(),
            }
        );
        assert_eq!(
            parse("/pages/about-us"),
            Route::PageDetails {
                slug: "about-us".into()
            }
        );
    }

    #[test]
    fn auth_routes_parse() {
        assert_eq!(parse("/login"), Route::Login {});
        assert_eq!(parse("/register"), Route::Register {});
        assert_eq!(parse("/logout"), Route::Logout {});
        assert_eq!(parse("/profile"), Route::EditProfile {});
        assert_eq!(parse("/profile/change-password"), Route::ChangePassword {});
    }

    #[test]
    fn admin_routes_parse() {
        assert_eq!(parse("/admin"), Route::Dashboard {});
        assert_eq!(parse("/admin/label"), Route::EditLabel {});
        assert_eq!(parse("/admin/artist"), Route::CreateArtist {});
        assert_eq!(
            parse("/admin/artist/jimi"),
            Route::EditArtist {
                artist_slug: "jimi".into()
            }
        );
        assert_eq!(
            parse("/admin/artist/jimi/links"),
            Route::EditArtistLinks {
                artist_slug: "jimi".into()
            }
        );
        assert_eq!(
            parse("/admin/artist/jimi/images"),
            Route::EditArtistImages {
                artist_slug: "jimi".into()
            }
        );
        assert_eq!(
            parse("/admin/artist/jimi/releases"),
            Route::Releases {
                artist_slug: "jimi".into()
            }
        );
        assert_eq!(
            parse("/admin/artist/jimi/releases/new"),
            Route::CreateRelease {
                artist_slug: "jimi".into()
            }
        );
        assert_eq!(
            parse("/admin/artist/jimi/release/axis"),
            Route::EditRelease {
                artist_slug: "jimi".into(),
                release_slug: "axis".into(),
            }
        );
        assert_eq!(
            parse("/admin/artist/jimi/release/axis/tracks"),
            Route::Tracks {
                artist_slug: "jimi".into(),
                release_slug: "axis".into(),
            }
        );
        assert_eq!(
            parse("/admin/artist/jimi/release/axis/tracks/new"),
            Route::CreateTrack {
                artist_slug: "jimi".into(),
                release_slug: "axis".into(),
            }
        );
        assert_eq!(
            parse("/admin/artist/jimi/release/axis/track/castles"),
            Route::EditTrack {
                artist_slug: "jimi".into(),
                release_slug: "axis".into(),
                track_slug: "castles".into(),
            }
        );
        assert_eq!(parse("/admin/page"), Route::CreatePage {});
        assert_eq!(
            parse("/admin/page/terms-of-use"),
            Route::EditPage {
                slug: "terms-of-use".into()
            }
        );
    }

    /// The Leptos table needed exact `""`/`"/"` route pairs; dioxus matches
    /// both spellings against one variant (MIGRATION.md risk 4).
    #[test]
    fn trailing_slashes_are_tolerated() {
        assert_eq!(parse("/admin/"), Route::Dashboard {});
        assert_eq!(parse("/artists/"), Route::ArtistsPage {});
        assert_eq!(
            parse("/artists/jimi/"),
            Route::ArtistPage {
                slug: "jimi".into()
            }
        );
        assert_eq!(
            parse("/admin/artist/jimi/"),
            Route::EditArtist {
                artist_slug: "jimi".into()
            }
        );
        assert_eq!(
            parse("/admin/artist/jimi/release/axis/tracks/"),
            Route::Tracks {
                artist_slug: "jimi".into(),
                release_slug: "axis".into(),
            }
        );
    }

    #[test]
    fn unmatched_paths_fall_back_to_not_found() {
        // Static artist sub-paths are exact-match; an unknown word after a
        // slug is the same ambiguity the Leptos table had.
        assert!(matches!(parse("/nope"), Route::NotFound { .. }));
        assert!(matches!(
            parse("/this/does/not/exist"),
            Route::NotFound { .. }
        ));
        assert!(matches!(
            parse("/admin/artist/jimi/bogus/deep"),
            Route::NotFound { .. }
        ));
        assert!(matches!(parse("/admin/unknown"), Route::NotFound { .. }));
    }

    #[test]
    fn canonical_urls_round_trip() {
        let routes = [
            Route::RecordLabelHome {},
            Route::PageDetails {
                slug: "cookies".into(),
            },
            // The nest-base route renders its canonical trailing slash and
            // still parses back (the quirk recorded in the Phase 0 spike).
            Route::EditArtist {
                artist_slug: "jimi".into(),
            },
            Route::EditTrack {
                artist_slug: "jimi".into(),
                release_slug: "axis".into(),
                track_slug: "castles".into(),
            },
        ];
        for route in routes {
            assert_eq!(
                parse(&route.to_string()),
                route,
                "canonical URL for {route:?} should parse back"
            );
        }
    }
}
