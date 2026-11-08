//! The application route table.
//!
//! Layouts wrap the variants below them (until `#[end_layout]`) and render
//! their child through `Outlet::<Route>`.
//!
//! Example — `/admin/artist/jimi/release/axis/tracks` renders `AppShell` >
//! `AdminRoot` > `AdminArtistRoot` > `Tracks`.
//!
//! One variant matches both `/x` and `/x/`, so the table needs no
//! trailing-slash duplicates (verified below and by the SSR tests in
//! `crate::server`).

use dioxus::prelude::*;

use super::admin::artist_root::AdminArtistRoot;
use super::admin::{
    AdminRoot, CreateArtist, CreatePage, CreateRelease, CreateTrack, Dashboard, EditArtist,
    EditArtistImages, EditArtistLinks, EditLabel, EditPage, EditRelease, EditTrack, Releases,
    Tracks,
};
use super::artist::{ArtistPage, ArtistsPage};
use super::auth::{ChangePassword, EditProfile, Login, Logout, Register};
use super::page::PageDetails;
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
    /// Log in. `bad` carries a fixed error code when a login attempt
    /// failed (the form posts to `/login`; see `crate::server::forms`).
    #[route("/login?:bad")]
    Login {
        /// Error code from a failed login attempt.
        bad: Option<String>,
    },
    /// Create an account. `bad` carries a fixed code when registration
    /// failed.
    #[route("/register?:bad")]
    Register {
        /// Error code from a failed registration attempt.
        bad: Option<String>,
    },
    /// Destroy the session. `done` is set by the redirect after a
    /// successful logout, which swaps the button for a "Logged out" line.
    #[route("/logout?:done")]
    Logout {
        /// Set when the session has just ended.
        done: Option<String>,
    },
    /// Edit the signed-in user's profile. `saved` and `bad` carry fixed
    /// codes from the profile form's redirect; `uploaded` and `bad_upload`
    /// carry the avatar upload form's (uploading without javascript).
    #[route("/profile?:saved&:bad&:uploaded&:bad_upload")]
    EditProfile {
        /// Set when the profile has just been saved.
        saved: Option<String>,
        /// Error code from a failed save.
        bad: Option<String>,
        /// Set when an avatar has just been uploaded without javascript.
        uploaded: Option<String>,
        /// Error code from a failed javascript-off upload.
        bad_upload: Option<String>,
    },
    /// Change the signed-in user's password. `updated` and `bad` carry
    /// fixed codes from the password form's redirect.
    #[route("/profile/change-password?:updated&:bad")]
    ChangePassword {
        /// Set when the password has just been changed.
        updated: Option<String>,
        /// Error code from a failed change.
        bad: Option<String>,
    },
    /// Admin section: menu + permission gate live in the layout.
    #[layout(AdminRoot)]
    /// Admin dashboard. `ready`, `purged`, `skipped` and `bad` carry the
    /// housekeeping purge form's fixed codes from its redirect (posts to
    /// `/admin/forms/purge`; see `crate::services::purge`).
    #[route("/admin?:ready&:purged&:skipped&:bad")]
    Dashboard {
        /// Row count from a purge preview ("this many would go").
        ready: Option<String>,
        /// Row count from a purge that ran.
        purged: Option<String>,
        /// Past-window artists a ran purge left in place.
        skipped: Option<String>,
        /// Error code from a refused purge.
        bad: Option<String>,
    },
    /// Edit the record label's details. `updated` and `bad` carry fixed
    /// codes from the label form's redirect (posts to
    /// `/admin/forms/label`; see `crate::server::forms`).
    #[route("/admin/label?:updated&:bad")]
    EditLabel {
        /// Set when the label has just been saved.
        updated: Option<String>,
        /// Error code from a failed save.
        bad: Option<String>,
    },
    /// Artist management under `/admin/artist`. The slug sits in the nest
    /// path, so every variant inside carries it as a prop — the layout
    /// receives it too (which is how `AdminArtistRoot` renders its tabs).
    /// `CreateArtist` lives outside the nest: it has no slug to carry.
    /// `bad` carries a fixed code from a failed create (the form posts to
    /// `/admin/forms/artist`).
    #[route("/admin/artist?:bad")]
    CreateArtist {
        /// Error code from a failed create.
        bad: Option<String>,
    },
    #[nest("/admin/artist/:artist_slug")]
    /// Artist editor tabs (profile/releases/links/images) wrapping the
    /// per-artist routes below.
    #[layout(AdminArtistRoot)]
    /// Edit an artist's details. Canonical URL ends with a slash
    /// (`/admin/artist/jimi/`); the form without one still parses.
    /// `updated`/`bad` carry the profile form's codes (posts to
    /// `/admin/forms/artist/*`; see `crate::server::forms`).
    #[route("/?:updated&:bad")]
    EditArtist {
        /// The artist slug.
        artist_slug: String,
        /// Set when the artist has just been saved/deleted/restored.
        updated: Option<String>,
        /// Error code from a failed save/delete/restore.
        bad: Option<String>,
    },
    /// Edit an artist's external links. `updated`/`bad` carry the links
    /// form's codes (posts to `/admin/forms/links`; see
    /// `crate::server::forms`).
    #[route("/links?:updated&:bad")]
    EditArtistLinks {
        /// The artist slug.
        artist_slug: String,
        /// Set when the links have just been saved.
        updated: Option<String>,
        /// Error code from a failed save.
        bad: Option<String>,
    },
    /// Upload/replace an artist's images. `uploaded`/`bad_upload` carry the
    /// upload form's codes (posts to `/forms/upload`; see
    /// `crate::server::upload_form`).
    #[route("/images?:uploaded&:bad_upload")]
    EditArtistImages {
        /// The artist slug.
        artist_slug: String,
        /// Set when an image has just been uploaded without javascript.
        uploaded: Option<String>,
        /// Error code from a failed javascript-off upload.
        bad_upload: Option<String>,
    },
    /// List an artist's releases.
    #[route("/releases")]
    Releases {
        /// The artist slug.
        artist_slug: String,
    },
    /// Create a release for an artist. `bad` carries a fixed code from a
    /// failed create (the form posts to `/admin/forms/release`).
    #[route("/releases/new?:bad")]
    CreateRelease {
        /// The artist slug.
        artist_slug: String,
        /// Error code from a failed create.
        bad: Option<String>,
    },
    /// Edit a release. `updated`/`bad` carry the release form's codes
    /// (posts to `/admin/forms/release/*`; see `crate::server::forms`),
    /// `uploaded`/`bad_upload` the image upload form's (`/forms/upload`).
    #[route("/release/:release_slug?:updated&:bad&:uploaded&:bad_upload")]
    EditRelease {
        /// The artist slug.
        artist_slug: String,
        /// The release slug.
        release_slug: String,
        /// Set when the release has just been saved/deleted/restored.
        updated: Option<String>,
        /// Error code from a failed save/delete/restore.
        bad: Option<String>,
        /// Set when an image has just been uploaded without javascript.
        uploaded: Option<String>,
        /// Error code from a failed javascript-off upload.
        bad_upload: Option<String>,
    },
    /// List a release's tracks.
    #[route("/release/:release_slug/tracks")]
    Tracks {
        /// The artist slug.
        artist_slug: String,
        /// The release slug.
        release_slug: String,
    },
    /// Create a track on a release. `bad` carries a fixed code from a
    /// failed create (the form posts to `/admin/forms/track`).
    #[route("/release/:release_slug/tracks/new?:bad")]
    CreateTrack {
        /// The artist slug.
        artist_slug: String,
        /// The release slug.
        release_slug: String,
        /// Error code from a failed create.
        bad: Option<String>,
    },
    /// Edit a track. `updated`/`bad` carry the track form's codes (posts to
    /// `/admin/forms/track/*`; see `crate::server::forms`), `uploaded`/
    /// `bad_upload` the audio upload form's (`/forms/upload`).
    #[route("/release/:release_slug/track/:track_slug?:updated&:bad&:uploaded&:bad_upload")]
    EditTrack {
        /// The artist slug.
        artist_slug: String,
        /// The release slug.
        release_slug: String,
        /// The track slug.
        track_slug: String,
        /// Set when the track has just been saved/deleted/restored.
        updated: Option<String>,
        /// Error code from a failed save/delete/restore.
        bad: Option<String>,
        /// Set when audio has just been uploaded without javascript.
        uploaded: Option<String>,
        /// Error code from a failed javascript-off upload.
        bad_upload: Option<String>,
    },
    #[end_layout]
    #[end_nest]
    /// Create a marketing/legal page. `bad` carries a fixed code from a
    /// failed create (the form posts to `/admin/forms/page`).
    #[route("/admin/page?:bad")]
    CreatePage {
        /// Error code from a failed create.
        bad: Option<String>,
    },
    /// Edit a marketing/legal page. `updated` and `bad` carry fixed codes
    /// from the update/delete/restore redirects (the forms post under
    /// `/admin/forms/page/*`).
    #[route("/admin/page/:slug?:updated&:bad")]
    EditPage {
        /// The page slug.
        slug: String,
        /// Set when the page has just been saved, deleted or restored.
        updated: Option<String>,
        /// Error code from a failed save/delete/restore.
        bad: Option<String>,
    },
    #[end_layout]
    /// Catch-all 404 page.
    #[route("/:..segments")]
    NotFound {
        /// The unmatched path, split into segments.
        segments: Vec<String>,
    },
}

/// Navigates to `to` without a full page load, replacing the current entry.
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
        assert_eq!(parse("/login"), Route::Login { bad: None });
        assert_eq!(parse("/register"), Route::Register { bad: None });
        assert_eq!(parse("/logout"), Route::Logout { done: None });
        assert_eq!(
            parse("/profile"),
            Route::EditProfile {
                saved: None,
                bad: None,
                uploaded: None,
                bad_upload: None,
            }
        );
        // The javascript-off avatar upload 303s back to these codes
        // (`upload_form_codes_parse` covers the admin pages).
        assert_eq!(
            parse("/profile?uploaded=1"),
            Route::EditProfile {
                saved: None,
                bad: None,
                uploaded: Some("1".into()),
                bad_upload: None,
            }
        );
        assert_eq!(
            parse("/profile/change-password"),
            Route::ChangePassword {
                updated: None,
                bad: None
            }
        );
    }

    /// The auth forms post to the axum handlers in `crate::server::forms`,
    /// which 303 back to these pages carrying fixed status codes in the
    /// query string.
    #[test]
    fn auth_routes_carry_query_codes() {
        assert_eq!(
            parse("/login?bad=password"),
            Route::Login {
                bad: Some("password".into())
            }
        );
        assert_eq!(
            parse("/logout?done=1"),
            Route::Logout {
                done: Some("1".into())
            }
        );
        assert_eq!(
            parse("/profile?saved=1"),
            Route::EditProfile {
                saved: Some("1".into()),
                bad: None,
                uploaded: None,
                bad_upload: None,
            }
        );
        assert_eq!(
            parse("/profile?bad=taken"),
            Route::EditProfile {
                saved: None,
                bad: Some("taken".into()),
                uploaded: None,
                bad_upload: None,
            }
        );
        assert_eq!(
            parse("/profile/change-password?updated=1"),
            Route::ChangePassword {
                updated: Some("1".into()),
                bad: None
            }
        );
    }

    /// The codes `server::upload_form` puts on its 303s parse back into the
    /// pages that host the upload widget.
    #[test]
    fn upload_form_codes_parse() {
        assert_eq!(
            parse("/admin/artist/jimi/images?uploaded=1"),
            Route::EditArtistImages {
                artist_slug: "jimi".into(),
                uploaded: Some("1".into()),
                bad_upload: None,
            }
        );
        assert_eq!(
            parse("/admin/artist/jimi/images?bad_upload=bad_mime"),
            Route::EditArtistImages {
                artist_slug: "jimi".into(),
                uploaded: None,
                bad_upload: Some("bad_mime".into()),
            }
        );
        assert_eq!(
            parse("/admin/artist/jimi/release/axis?uploaded=1"),
            Route::EditRelease {
                artist_slug: "jimi".into(),
                release_slug: "axis".into(),
                updated: None,
                bad: None,
                uploaded: Some("1".into()),
                bad_upload: None,
            }
        );
        assert_eq!(
            parse("/profile?bad_upload=too_large"),
            Route::EditProfile {
                saved: None,
                bad: None,
                uploaded: None,
                bad_upload: Some("too_large".into()),
            }
        );
    }

    #[test]
    fn admin_routes_parse() {
        assert_eq!(
            parse("/admin"),
            Route::Dashboard {
                ready: None,
                purged: None,
                skipped: None,
                bad: None
            }
        );
        assert_eq!(
            parse("/admin/label"),
            Route::EditLabel {
                updated: None,
                bad: None
            }
        );
        assert_eq!(parse("/admin/artist"), Route::CreateArtist { bad: None });
        assert_eq!(
            parse("/admin/artist/jimi"),
            Route::EditArtist {
                artist_slug: "jimi".into(),
                updated: None,
                bad: None,
            }
        );
        assert_eq!(
            parse("/admin/artist/jimi/links"),
            Route::EditArtistLinks {
                artist_slug: "jimi".into(),
                updated: None,
                bad: None,
            }
        );
        assert_eq!(
            parse("/admin/artist/jimi/images"),
            Route::EditArtistImages {
                artist_slug: "jimi".into(),
                uploaded: None,
                bad_upload: None,
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
                artist_slug: "jimi".into(),
                bad: None,
            }
        );
        assert_eq!(
            parse("/admin/artist/jimi/release/axis"),
            Route::EditRelease {
                artist_slug: "jimi".into(),
                release_slug: "axis".into(),
                updated: None,
                bad: None,
                uploaded: None,
                bad_upload: None,
            }
        );
        assert_eq!(parse("/admin/page"), Route::CreatePage { bad: None });
        assert_eq!(
            parse("/admin/page/terms-of-use"),
            Route::EditPage {
                slug: "terms-of-use".into(),
                updated: None,
                bad: None,
            }
        );
    }

    /// The routes under a release (its track list, its track editors) parse
    /// with the artist and release slugs the nest carries.
    #[test]
    fn nested_release_routes_parse() {
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
                bad: None,
            }
        );
        assert_eq!(
            parse("/admin/artist/jimi/release/axis/track/castles"),
            Route::EditTrack {
                artist_slug: "jimi".into(),
                release_slug: "axis".into(),
                track_slug: "castles".into(),
                updated: None,
                bad: None,
                uploaded: None,
                bad_upload: None,
            }
        );
    }

    /// The admin forms 303 back to these pages with fixed status codes in
    /// the query string (the auth-code pattern).
    #[test]
    fn admin_routes_carry_query_codes() {
        assert_eq!(
            parse("/admin/artist?bad=name_required"),
            Route::CreateArtist {
                bad: Some("name_required".into())
            }
        );
        // Nest base carries the slug, the child the codes (both spellings parse).
        assert_eq!(
            parse("/admin/artist/jimi?updated=1"),
            Route::EditArtist {
                artist_slug: "jimi".into(),
                updated: Some("1".into()),
                bad: None,
            }
        );
        assert_eq!(
            parse("/admin/artist/jimi/?bad=slug_taken"),
            Route::EditArtist {
                artist_slug: "jimi".into(),
                updated: None,
                bad: Some("slug_taken".into()),
            }
        );
        assert_eq!(
            parse("/admin/label?updated=1"),
            Route::EditLabel {
                updated: Some("1".into()),
                bad: None,
            }
        );
        assert_eq!(
            parse("/admin/label?bad=forbidden"),
            Route::EditLabel {
                updated: None,
                bad: Some("forbidden".into()),
            }
        );
        assert_eq!(
            parse("/admin/page?bad=name_required"),
            Route::CreatePage {
                bad: Some("name_required".into())
            }
        );
        assert_eq!(
            parse("/admin/page/terms-of-use?updated=1"),
            Route::EditPage {
                slug: "terms-of-use".into(),
                updated: Some("1".into()),
                bad: None,
            }
        );
        assert_eq!(
            parse("/admin/page/terms-of-use?bad=slug_taken"),
            Route::EditPage {
                slug: "terms-of-use".into(),
                updated: None,
                bad: Some("slug_taken".into()),
            }
        );
    }

    /// Release- and track-level routes address their parents by slug and
    /// carry their own query codes.
    #[test]
    fn nested_admin_routes_carry_query_codes() {
        assert_eq!(
            parse("/admin/artist/jimi/releases/new?bad=slug_taken"),
            Route::CreateRelease {
                artist_slug: "jimi".into(),
                bad: Some("slug_taken".into()),
            }
        );
        assert_eq!(
            parse("/admin/artist/jimi/release/axis?updated=1"),
            Route::EditRelease {
                artist_slug: "jimi".into(),
                release_slug: "axis".into(),
                updated: Some("1".into()),
                bad: None,
                uploaded: None,
                bad_upload: None,
            }
        );
        assert_eq!(
            parse("/admin/artist/jimi/release/axis?bad=catalogue_taken"),
            Route::EditRelease {
                artist_slug: "jimi".into(),
                release_slug: "axis".into(),
                updated: None,
                bad: Some("catalogue_taken".into()),
                uploaded: None,
                bad_upload: None,
            }
        );
        assert_eq!(
            parse("/admin/artist/jimi/release/axis/tracks/new?bad=isrc_taken"),
            Route::CreateTrack {
                artist_slug: "jimi".into(),
                release_slug: "axis".into(),
                bad: Some("isrc_taken".into()),
            }
        );
        assert_eq!(
            parse("/admin/artist/jimi/release/axis/track/castles?updated=1"),
            Route::EditTrack {
                artist_slug: "jimi".into(),
                release_slug: "axis".into(),
                track_slug: "castles".into(),
                updated: Some("1".into()),
                bad: None,
                uploaded: None,
                bad_upload: None,
            }
        );
        // The audio upload's codes ride the same route.
        assert_eq!(
            parse("/admin/artist/jimi/release/axis/track/castles?uploaded=1"),
            Route::EditTrack {
                artist_slug: "jimi".into(),
                release_slug: "axis".into(),
                track_slug: "castles".into(),
                updated: None,
                bad: None,
                uploaded: Some("1".into()),
                bad_upload: None,
            }
        );
        assert_eq!(
            parse("/admin/artist/jimi/release/axis/track/castles?bad_upload=bad_mime"),
            Route::EditTrack {
                artist_slug: "jimi".into(),
                release_slug: "axis".into(),
                track_slug: "castles".into(),
                updated: None,
                bad: None,
                uploaded: None,
                bad_upload: Some("bad_mime".into()),
            }
        );
        assert_eq!(
            parse("/admin/artist/jimi/links?bad=forbidden"),
            Route::EditArtistLinks {
                artist_slug: "jimi".into(),
                updated: None,
                bad: Some("forbidden".into()),
            }
        );
    }

    /// One variant matches both the bare and trailing-slash spellings of a
    /// path, so both serve the same page.
    #[test]
    fn trailing_slashes_are_tolerated() {
        assert_eq!(
            parse("/admin/"),
            Route::Dashboard {
                ready: None,
                purged: None,
                skipped: None,
                bad: None
            }
        );
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
                artist_slug: "jimi".into(),
                updated: None,
                bad: None,
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
        // slug falls through to NotFound.
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
            // still parses back; the short form with the profile form's
            // codes round-trips too.
            Route::EditArtist {
                artist_slug: "jimi".into(),
                updated: None,
                bad: None,
            },
            Route::EditArtist {
                artist_slug: "jimi".into(),
                updated: Some("1".into()),
                bad: None,
            },
            Route::CreateRelease {
                artist_slug: "jimi".into(),
                bad: Some("forbidden".into()),
            },
            Route::EditRelease {
                artist_slug: "jimi".into(),
                release_slug: "axis".into(),
                updated: Some("1".into()),
                bad: None,
                uploaded: None,
                bad_upload: None,
            },
            Route::CreateTrack {
                artist_slug: "jimi".into(),
                release_slug: "axis".into(),
                bad: Some("forbidden".into()),
            },
            Route::EditTrack {
                artist_slug: "jimi".into(),
                release_slug: "axis".into(),
                track_slug: "castles".into(),
                updated: Some("1".into()),
                bad: None,
                uploaded: None,
                bad_upload: None,
            },
            Route::EditTrack {
                artist_slug: "jimi".into(),
                release_slug: "axis".into(),
                track_slug: "castles".into(),
                updated: None,
                bad: None,
                uploaded: None,
                bad_upload: None,
            },
            Route::EditTrack {
                artist_slug: "jimi".into(),
                release_slug: "axis".into(),
                track_slug: "castles".into(),
                updated: None,
                bad: None,
                uploaded: Some("1".into()),
                bad_upload: None,
            },
            Route::EditArtistLinks {
                artist_slug: "jimi".into(),
                updated: Some("1".into()),
                bad: None,
            },
            Route::EditArtistLinks {
                artist_slug: "jimi".into(),
                updated: None,
                bad: Some("forbidden".into()),
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
