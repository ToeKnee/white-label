//! How the app addresses static files (MIGRATION.md Phase 6 slice 6b).
//!
//! Web and SSR pages use root-relative paths, which the SSR server answers
//! from its public dir and its `/uploads` mount. A native window has no such
//! server: its own origin (`dioxus://index.html/`, shown to the page as
//! `http://dioxus.index.html/`) serves only what the bundle holds, and that
//! resolver looks files up under the bundle's `assets/` directory
//! (`dioxus-asset-resolver::native::resolve_asset_path_from_filesystem`). So
//! the two kinds of file need different addresses on desktop:
//!
//! - files shipped *with the app* (`main.css`, the service icons) live in the
//!   bundle: [`crate::ui::assets::bundled_asset`] points at `/assets/…`, and
//!   the build copies `public/` into the bundle's `assets/` directory (`dx`
//!   only bundles files registered with `asset!()`, and we register none, so
//!   without that copy the window renders unstyled);
//! - files that live *on the API server* (`/uploads/…`: uploaded artist,
//!   release, track and avatar images) are fetched over HTTP from the origin
//!   the launcher gave `set_server_url`: [`crate::ui::assets::served_file_url`].
//!   They are plain `<img>` subresources, so no CORS is involved.
//!
//! The desktop bundle is a copy of `public/`, so the icons and CSS are read
//! from disk on every window; a release bundle ships them inside the app.

/// Path for a file shipped with the app bundle.
///
/// Native bundles keep their files under `assets/`; web and SSR serve them
/// from the public dir root.
pub fn bundled_asset(path: &str) -> String {
    bundled_path(cfg!(feature = "desktop"), path)
}

/// URL for a file the API server serves, e.g. `/uploads/artists/…`.
///
/// Native builds prefix the API origin (the webview's own origin has no
/// uploads); web and SSR keep the root-relative path.
pub fn served_file_url(path: &str) -> String {
    absolute(api_origin(), path)
}

/// URL for whatever a model's `primary_image_url`/`avatar_url` returned.
///
/// Those helpers yield either an `/uploads/…` path (a file the API server
/// holds) or the `/Logo.svg` placeholder (a static that ships with the app),
/// so the two buckets are sorted here rather than at every `<img>`.
pub fn display_image_url(image_url: &str) -> String {
    if image_url.starts_with("/uploads/") {
        served_file_url(image_url)
    } else {
        bundled_asset(image_url)
    }
}

/// The API origin for this build, with any trailing slash removed. Empty for
/// targets whose page origin *is* the server (web, SSR).
// One signature covers both targets; only the native one reads anything, so
// the non-desktop branch trips `missing_const_for_fn`.
#[cfg_attr(
    not(feature = "desktop"),
    expect(
        clippy::missing_const_for_fn,
        reason = "the desktop branch reads the launch-time server URL, which is not const; one signature for both targets is clearer than two functions"
    )
)]
fn api_origin() -> &'static str {
    // `get_server_url` is the value the launcher passed to `set_server_url`;
    // only the native launchers set one.
    #[cfg(feature = "desktop")]
    {
        dioxus::fullstack::get_server_url().trim_end_matches('/')
    }
    #[cfg(not(feature = "desktop"))]
    {
        ""
    }
}

/// Split out so both branches are testable whatever features are on.
fn bundled_path(native: bool, path: &str) -> String {
    let file_name = path.trim_start_matches('/');
    if native {
        format!("/assets/{file_name}")
    } else {
        format!("/{file_name}")
    }
}

/// Split out so the native join is testable without the `desktop` feature.
fn absolute(origin: &str, path: &str) -> String {
    format!("{origin}{path}")
}

#[cfg(test)]
mod tests {
    use super::{absolute, bundled_path, display_image_url};

    #[test]
    fn bundled_assets_stay_at_the_root_for_web_and_ssr() {
        assert_eq!(bundled_path(false, "/main.css"), "/main.css");
        assert_eq!(
            bundled_path(false, "/images/music_services/SoundCloud.svg"),
            "/images/music_services/SoundCloud.svg"
        );
    }

    #[test]
    fn bundled_assets_come_from_the_bundle_on_desktop() {
        assert_eq!(bundled_path(true, "/main.css"), "/assets/main.css");
        // A path without a leading slash still gets one.
        assert_eq!(bundled_path(true, "Logo.svg"), "/assets/Logo.svg");
    }

    #[test]
    fn served_files_are_root_relative_without_an_api_origin() {
        assert_eq!(
            absolute("", "/uploads/artists/1700000000-jimi.png"),
            "/uploads/artists/1700000000-jimi.png"
        );
    }

    #[test]
    fn served_files_carry_the_api_origin_on_desktop() {
        assert_eq!(
            absolute(
                "http://127.0.0.1:8127",
                "/uploads/artists/1700000000-jimi.png"
            ),
            "http://127.0.0.1:8127/uploads/artists/1700000000-jimi.png"
        );
        // The desktop launcher trims the trailing slash, so a configured
        // `http://host:8127/` never becomes `…//uploads`.
        assert_eq!(
            absolute(
                "https://label.example.com".trim_end_matches('/'),
                "/uploads/avatars/1700000000-tony.png"
            ),
            "https://label.example.com/uploads/avatars/1700000000-tony.png"
        );
    }

    // `display_image_url` is what the `<img>` call sites use: the models hand
    // back either kind of path, and this run only compiles the non-desktop
    // branch, so it pins the sorting rule for the web-facing targets.
    #[test]
    fn image_urls_sort_uploads_from_placeholders() {
        assert_eq!(
            display_image_url("/uploads/releases/1700000000-axis.jpg"),
            "/uploads/releases/1700000000-axis.jpg"
        );
        assert_eq!(display_image_url("/Logo.svg"), "/Logo.svg");
    }
}
