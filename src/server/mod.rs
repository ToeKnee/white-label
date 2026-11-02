//! Dioxus server side of the migration (see MIGRATION.md, Phases 1–3).
//!
//! Assembles the axum router that `dioxus::serve` wraps: the app database
//! pool as a request extension, `tower-sessions` backed by Postgres, static
//! uploads, the `/api/*` port and the Dioxus SSR app (`crate::ui`). The
//! Leptos `setup.rs` still serves the shipped site until cutover.

mod api;
pub mod current_user;

use anyhow::Context;
use axum::Router;
use tower_sessions::SessionManagerLayer;
use tower_sessions_sqlx_store::PostgresStore;

use crate::server::current_user::Db;

/// Postgres table holding Dioxus-server sessions. Kept separate from the
/// Leptos app's `axum_sessions` table — the two stacks use different
/// schemas, so during the overlap each server owns its own sessions (users
/// log in again once at cutover).
const SESSION_TABLE: &str = "sessions";

/// Uploads directory (`UPLOAD_PATH`), exposed to API handlers as a request
/// extension.
///
/// `None` when the server has no uploads dir. The Leptos route read the
/// env var inside the handler — passing it through the router instead
/// keeps the value injectable for tests.
#[derive(Clone, Debug)]
pub struct UploadRoot(Option<std::path::PathBuf>);

impl UploadRoot {
    /// Wrap the uploads directory (if the server has one).
    #[must_use]
    pub const fn new(path: Option<std::path::PathBuf>) -> Self {
        Self(path)
    }

    /// The root as a path string, for `{root}/{sub}/{file}` joins done the
    /// same string-concatenation way the Leptos routes do.
    #[must_use]
    pub fn root(&self) -> Option<String> {
        self.0.as_ref().map(|path| path.display().to_string())
    }
}

/// Build the Dioxus server's router (database, sessions, uploads, Dioxus
/// SSR). Factored out from serving so `axum-test` can drive it without
/// binding a socket (Phase 2 exit criteria).
///
/// The caller supplies the pool and the uploads directory: the
/// `dioxus-server` binary wires these from the environment (`create_pool`,
/// `UPLOAD_PATH`), while tests pass a fresh `#[sqlx::test]` pool and a
/// temp dir.
///
/// # Errors
///
/// Returns an error if the session store can't be configured or migrated.
pub async fn build_router(
    pool: sqlx::PgPool,
    upload_path: Option<std::path::PathBuf>,
) -> anyhow::Result<Router> {
    let session_store = PostgresStore::new(pool.clone())
        .with_table_name(SESSION_TABLE)
        .map_err(|error| anyhow::anyhow!("invalid session table name: {error}"))?;
    session_store
        .migrate()
        .await
        .context("could not create/migrate the sessions table")?;

    let mut router = dioxus::server::router(crate::ui::App)
        // Ported `/api/*` endpoints (see `api` module); merged before the
        // layers below so handlers get the session and `Db` extensions.
        .merge(api::api_routes())
        .layer(SessionManagerLayer::new(session_store))
        // Handlers and server fns reach the pool via `Db`.
        .layer(axum::Extension(Db(pool)))
        // The upload endpoints (`api::files`) resolve their root here.
        .layer(axum::Extension(UploadRoot::new(upload_path.clone())));

    // Uploaded files, mounted after the session stack so static hits skip
    // the database — same treatment as the Leptos app's `/uploads`.
    if let Some(upload_path) = upload_path {
        router = router.nest_service("/uploads", tower_http::services::ServeDir::new(upload_path));
    }

    Ok(router)
}

/// `dioxus::server::router` reads (and serves) a `public` dir next to
/// the executable unless `DIOXUS_PUBLIC_PATH` overrides it. Create an
/// empty one for the test binary so the router can build; `target/`
/// is disposable, so leaving it behind is fine.
#[cfg(test)]
pub(crate) fn ensure_exe_public_dir() {
    let public_dir = std::env::current_exe()
        .expect("test binary path")
        .parent()
        .expect("target deps dir")
        .join("public");
    std::fs::create_dir_all(public_dir).expect("empty public dir for the dioxus router");
}

#[cfg(test)]
mod tests {
    use axum_test::{TestResponse, TestServer};
    use sqlx::PgPool;

    use super::ensure_exe_public_dir;
    use crate::models::test_helpers::{
        create_test_artist, create_test_page, create_test_record_label, create_test_release,
        create_test_track, create_test_user_with_permissions,
    };
    use crate::server::api::test_support::{api_server, post_args};
    use crate::server::build_router;

    /// Build the real router and wrap it for in-process HTTP.
    async fn test_server(pool: PgPool, upload_path: Option<std::path::PathBuf>) -> TestServer {
        ensure_exe_public_dir();
        let router = build_router(pool, upload_path)
            .await
            .expect("router should build against the test pool");
        TestServer::new(router).expect("axum-test should wrap the router")
    }

    /// Assert the response is 200 and its SSR body contains every marker.
    fn assert_shell(response: &TestResponse, markers: &[&str]) {
        response.assert_status_ok();
        let body = response.text();
        for marker in markers {
            assert!(
                body.contains(marker),
                "SSR body missing `{marker}`:\n{body}"
            );
        }
    }

    /// The navbar/footer chrome every route renders around the page body.
    const CHROME: &[&str] = &[
        "navbar bg-primary text-primary-content",
        "footer bg-neutral text-neutral-content",
    ];

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn root_serves_the_shell_and_head(pool: PgPool) {
        let server = test_server(pool, None).await;

        let response = server.get("/").await;

        // An empty database leaves the home page with empty label fields
        // (the fetches fail as they did for the Leptos store default), so
        // this test pins the shell and head elements only; seeded content
        // and titles are asserted in `public_pages_render_seeded_content`.
        assert_shell(
            &response,
            &[
                CHROME[0],
                CHROME[1],
                "</main>",
                "Register",
                "Login",
                "href=\"/main.css\"",
                "width=device-width, initial-scale=1",
            ],
        );
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn every_route_serves_its_page(pool: PgPool) {
        let server = test_server(pool, None).await;

        // (path, marker) — the marker is the placeholder page's h1 text.
        // The public pages render seeded data instead (see the tests
        // below); only the still-stubbed auth/admin routes are listed here.
        let routes = [
            ("/login", "Log in"),
            ("/register", "Register"),
            ("/logout", "Log out"),
            ("/profile", "Edit profile"),
            ("/profile/change-password", "Change password"),
            ("/admin", "Admin dashboard"),
            ("/admin/label", "Edit label"),
            ("/admin/artist", "Create artist"),
            ("/admin/page", "Create page"),
            ("/admin/page/terms-of-use", "Edit page"),
        ];
        for (path, marker) in routes {
            let response = server.get(path).await;
            assert_shell(&response, &[CHROME[0], CHROME[1], marker]);
        }
    }

    /// Seed one of every entity a public page renders: label 1, its
    /// published artist 1 (with release 1 and track 1) and page 1.
    async fn seed_public_site(pool: &PgPool) {
        let label = create_test_record_label(pool, 1)
            .await
            .expect("label fixture");
        let artist = create_test_artist(pool, 1, Some(label.clone()))
            .await
            .expect("artist fixture");
        let release = create_test_release(pool, 1, Some(artist.clone()))
            .await
            .expect("release fixture");
        create_test_track(pool, 1, Some(release), Some(artist))
            .await
            .expect("track fixture");
        create_test_page(pool, 1, Some(label))
            .await
            .expect("page fixture");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn public_pages_render_seeded_content(pool: PgPool) {
        seed_public_site(&pool).await;
        let server = test_server(pool, None).await;

        // Home: the label name as the page title and h1, its markdown
        // description, and the published artist's card.
        let home = server.get("/").await;
        assert_shell(
            &home,
            &[
                CHROME[0],
                CHROME[1],
                "<title>Test Record Label 1</title>",
                "A record label for testing purposes with the id of 1",
                "Test Artist 1",
            ],
        );

        // Artists index: the "{label} Artists" heading and the card.
        let artists = server.get("/artists").await;
        assert_shell(
            &artists,
            &[CHROME[0], "Test Record Label 1 Artists", "Test Artist 1"],
        );

        // Artist page: bio text, website link, and the release card.
        let artist = server.get("/artists/test-artist-1").await;
        assert_shell(
            &artist,
            &[
                CHROME[0],
                "A artist for testing purposes with the id of 1",
                "https://test-artist-1.example.com",
                "Test Release 1",
            ],
        );

        // Release page: description and the track row.
        let release = server.get("/artists/test-artist-1/test-release-1").await;
        assert_shell(
            &release,
            &[
                CHROME[0],
                "A release for testing purposes with the id of 1",
                "Test Track 1",
            ],
        );

        // Marketing page: the page body rendered as markdown (the fixture
        // body is an `#` heading, so the marker spans the rendered <h1>).
        let page = server.get("/pages/test-page-1").await;
        assert_shell(
            &page,
            &[
                CHROME[0],
                "Test Page 1",
                ">A page for testing purposes with the id of 1<",
            ],
        );
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn release_page_redirects_for_unknown_slugs(pool: PgPool) {
        seed_public_site(&pool).await;
        let server = test_server(pool, None).await;

        // Unknown artist: the artists index takes over (the Leptos
        // `redirect("/artists")` equivalent, issued from the render path).
        let unknown_artist = server.get("/artists/nobody/nonsense").await;
        assert_shell(&unknown_artist, &[CHROME[0], "Test Record Label 1 Artists"]);

        // Known artist, unknown release: the artist page takes over.
        let unknown_release = server.get("/artists/test-artist-1/nonsense").await;
        assert_shell(&unknown_release, &[CHROME[0], "Test Release 1"]);
    }

    /// The body `POST /api/login` expects (the wire args minus the optional
    /// `remember` flag).
    #[derive(serde::Serialize)]
    struct LoginArgs<'a> {
        username: &'a str,
        password: &'a str,
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn label_owner_sessions_see_hidden_artists(pool: PgPool) {
        let label = create_test_record_label(&pool, 1)
            .await
            .expect("label fixture");
        create_test_artist(&pool, 1, Some(label.clone()))
            .await
            .expect("published artist fixture");
        // A second, unpublished artist — same insert as the fixture helper
        // but with no `published_at`.
        sqlx::query("INSERT INTO artists (name, slug, description, website, label_id, published_at) VALUES ($1, $2, $3, $4, $5, $6)")
            .bind("Hidden Artist")
            .bind("hidden-artist")
            .bind("Not published yet")
            .bind("")
            .bind(label.id)
            .bind(Option::<chrono::DateTime<chrono::Utc>>::None)
            .execute(&pool)
            .await
            .expect("hidden artist fixture");
        create_test_user_with_permissions(&pool, 2, vec!["label_owner"])
            .await
            .expect("label_owner user fixture");

        // `save_cookies` carries the login session into the SSR requests —
        // the extractor side of slice 2's data loading.
        let server = api_server(pool).await;

        let guest_body = server.get("/artists").await.text();
        assert!(guest_body.contains("Test Artist 1"), "{guest_body}");
        assert!(
            !guest_body.contains("Hidden Artist"),
            "guests must not see unpublished artists:\n{guest_body}"
        );

        post_args(
            &server,
            "/api/login",
            &LoginArgs {
                username: "test-2",
                password: "password",
            },
        )
        .await
        .assert_status_ok();

        let owner_body = server.get("/artists").await.text();
        assert!(
            owner_body.contains("Hidden Artist"),
            "label_owner sessions should see unpublished artists:\n{owner_body}"
        );
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn nested_admin_layouts_render_outside_in(pool: PgPool) {
        let server = test_server(pool, None).await;

        let response = server.get("/admin/artist/jimi").await;

        // AppShell chrome, AdminRoot wrapper, AdminArtistRoot tabs, and the
        // page itself — with the slug prop reaching the child component.
        assert_shell(
            &response,
            &[
                CHROME[0],
                CHROME[1],
                "tabs tabs-border",
                "href=\"/admin/artist/jimi/releases\"",
                "tab tab-active",
                "slug: jimi",
            ],
        );
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn deep_nested_routes_and_trailing_slashes_serve(pool: PgPool) {
        let server = test_server(pool, None).await;

        let deep = server
            .get("/admin/artist/jimi/release/axis/track/castles")
            .await;
        assert_shell(&deep, &["Edit track", "track: castles"]);

        // The exact `"x"`/`"x/"` route pairs the Leptos table needed are one
        // variant here (MIGRATION.md risk 4).
        let slashed = server.get("/admin/artist/jimi/").await;
        assert_shell(&slashed, &["Edit artist", "slug: jimi"]);
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn unmatched_paths_render_the_not_found_page(pool: PgPool) {
        let server = test_server(pool, None).await;

        let response = server.get("/totally/bogus/path").await;

        // The catch-all still renders inside the shell, like the Leptos
        // `Routes fallback` did; the 404 *page* is served with 200 there
        // too. Apostrophes in text are HTML-escaped by the renderer, hence
        // the apostrophe-free marker.
        assert_shell(&response, &[CHROME[0], "find what you were looking for!"]);
    }

    #[sqlx::test]
    async fn build_migrates_the_sessions_table(pool: PgPool) {
        // Keep a handle on the pool: `build_router` moves its clone into
        // the session store and the `Db` extension.
        let check_pool = pool.clone();
        ensure_exe_public_dir();
        let router = build_router(pool, None)
            .await
            .expect("router should build against the test pool");
        drop(router);

        let migrated: bool =
            sqlx::query_scalar("select to_regclass('tower_sessions.sessions') is not null")
                .fetch_one(&check_pool)
                .await
                .expect("catalogue query should run on the test db");
        assert!(
            migrated,
            "`build_router` should create the tower_sessions.sessions table"
        );
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn uploaded_files_are_served(pool: PgPool) {
        let upload_dir = tempfile::tempdir().expect("temp uploads dir");
        std::fs::write(upload_dir.path().join("sample.txt"), "upload bytes")
            .expect("sample upload file");

        let server = test_server(pool, Some(upload_dir.path().to_path_buf())).await;

        let hit = server.get("/uploads/sample.txt").await;
        hit.assert_status_ok();
        assert_eq!(hit.text(), "upload bytes");

        server
            .get("/uploads/missing.txt")
            .await
            .assert_status_not_found();
    }
}
