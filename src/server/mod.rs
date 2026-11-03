//! Dioxus server side of the migration (see MIGRATION.md, Phases 1–3).
//!
//! Assembles the axum router that `dioxus::serve` wraps: the app database
//! pool as a request extension, `tower-sessions` backed by Postgres,
//! static uploads, the `/api/*` port, the auth form handlers
//! (`forms`) and the Dioxus SSR app (`crate::ui`). The Leptos `setup.rs`
//! still serves the shipped site until cutover.

mod api;
pub mod current_user;
mod forms;

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
        // The auth pages' form posts (see `forms`), POST counterparts of
        // the SSR pages' paths, also under the session and `Db` layers.
        .merge(forms::form_routes())
        .layer(SessionManagerLayer::new(session_store))
        // Handlers and server fns reach the pool via `Db`.
        .layer(axum::Extension(Db(pool)))
        // The upload endpoints (`api::files`) resolve their root here.
        .layer(axum::Extension(UploadRoot::new(upload_path.clone())));

    // Uploaded files, mounted after the session stack so static hits skip
    // the database — same treatment as the Leptos app's `/uploads`.
    //
    // Dioxus' own server router nests every top-level entry of the public
    // dir (`serve_dir_cached` in dioxus-server), so a public `uploads`
    // entry already occupies `/uploads` and nesting there again panics
    // axum's route table. `uploads_mount` turns that into a skip (the
    // uploads root *is* the public entry) or a clear startup error.
    if let Some(upload_path) = upload_path {
        match uploads_mount(&upload_path, dioxus_public_dir().as_deref()) {
            UploadsMount::Skip => {
                tracing::warn!(
                    "uploads root is the public dir's uploads entry; dioxus serves it at /uploads, skipping the explicit mount"
                );
            }
            UploadsMount::Conflict => {
                return Err(anyhow::anyhow!(
                    "the dioxus public dir contains an `uploads` entry, which dioxus mounts at /uploads; that collides with the uploads root {} — remove it from the public dir or point UPLOAD_PATH at it",
                    upload_path.display()
                ));
            }
            UploadsMount::Mount => {
                router = router
                    .nest_service("/uploads", tower_http::services::ServeDir::new(upload_path));
            }
        }
    }

    Ok(router)
}

/// The public dir dioxus' server router will serve, mirroring the private
/// `public_path()` in dioxus-server: `DIOXUS_PUBLIC_PATH` first, else the
/// `public` dir next to the executable.
fn dioxus_public_dir() -> Option<std::path::PathBuf> {
    if let Ok(path) = std::env::var("DIOXUS_PUBLIC_PATH") {
        return Some(path.into());
    }
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    Some(exe_dir.join("public"))
}

/// What to do about the explicit `/uploads` mount (see the call site).
#[derive(Debug, PartialEq, Eq)]
enum UploadsMount {
    /// Dioxus already serves the uploads root itself (it *is* the public
    /// dir's `uploads` entry).
    Skip,
    /// A public `uploads` entry blocks `/uploads` for a different uploads
    /// root; nesting would panic axum's route table.
    Conflict,
    /// Nest the uploads root at `/uploads` as usual.
    Mount,
}

/// Decide the `/uploads` mount from the configured uploads root and the
/// public dir dioxus serves. The skip comparison requires both paths to
/// exist (canonicalize), so a mistyped `UPLOAD_PATH` never skips silently.
fn uploads_mount(
    upload_path: &std::path::Path,
    public_dir: Option<&std::path::Path>,
) -> UploadsMount {
    let public_uploads = public_dir.map(|dir| dir.join("uploads"));
    match (
        upload_path.canonicalize(),
        public_uploads.as_deref().map(std::path::Path::canonicalize),
    ) {
        (Ok(upload), Some(Ok(auto))) if upload == auto => UploadsMount::Skip,
        // Dioxus nests any existing public `uploads` entry (dir or file),
        // which would clash with our own `/uploads` catch-all.
        _ if public_uploads.is_some_and(|entry| entry.exists()) => UploadsMount::Conflict,
        _ => UploadsMount::Mount,
    }
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
    use axum::http::StatusCode;
    use axum_test::{TestResponse, TestServer};
    use sqlx::PgPool;

    use super::ensure_exe_public_dir;
    use super::{UploadsMount, uploads_mount};
    use crate::models::test_helpers::{
        create_test_artist, create_test_page, create_test_record_label, create_test_release,
        create_test_track, create_test_user, create_test_user_with_permissions,
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
    async fn guests_visiting_admin_routes_get_the_login_form(pool: PgPool) {
        let server = test_server(pool, None).await;

        // (slice 4) The `AdminRoot` layout gates every admin route: guests
        // get the login page rendered in place of the guarded route (the
        // slice-3 pattern; the redirect swaps the route out of the SSR
        // render, so the admin menu never appears). The placeholder
        // artist/release/track pages behind the gate are asserted with an
        // admin session below.
        let routes = [
            "/admin",
            "/admin/label",
            "/admin/artist",
            "/admin/page",
            "/admin/page/terms-of-use",
            "/admin/artist/jimi/releases",
        ];
        for path in routes {
            let response = server.get(path).await;
            assert_shell(
                &response,
                &[
                    CHROME[0],
                    CHROME[1],
                    "<h1>Login</h1>",
                    "action=\"/auth/login\"",
                ],
            );
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
    async fn unmatched_artist_routes_still_serve_the_shell(pool: PgPool) {
        seed_public_site(&pool).await;
        let server = test_server(pool, None).await;

        // A nested slug that matches no route: the artists index still
        // renders from the nest parent; the 404 appears deeper in the body.
        let unknown_artist = server.get("/artists/nobody/nonsense").await;
        assert_shell(&unknown_artist, &[CHROME[0], "Test Record Label 1 Artists"]);

        // `/artists/test-artist-1/nonsense` is the *artist* route
        // (`nonsense` is an ignored extra segment), not a redirect — the
        // artist page renders and its release cards carry the label.
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

    // --- Auth form flows (MIGRATION.md Phase 4 slice 3) -----------------
    //
    // The pages in `crate::ui::auth` post to the 303-redirect handlers in
    // `crate::server::forms`; `api_server`'s cookie jar plays the browser.

    /// Post an `application/x-www-form-urlencoded` body, like a real form.
    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; these tests run on one thread via block_on, so Send isn't needed"
    )]
    async fn post_form(server: &TestServer, path: &str, fields: &[(&str, &str)]) -> TestResponse {
        let form: std::collections::BTreeMap<&str, &str> = fields.iter().copied().collect();
        server.post(path).form(&form).await
    }

    /// The `Location` header of a redirect, as a string.
    fn location(response: &TestResponse) -> String {
        response
            .header(axum::http::header::LOCATION)
            .to_str()
            .expect("utf-8 location header")
            .to_string()
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn auth_pages_render_their_forms(pool: PgPool) {
        let server = test_server(pool, None).await;

        assert_shell(
            &server.get("/login").await,
            &[CHROME[0], "action=\"/auth/login\"", "Remember me"],
        );
        assert_shell(
            &server.get("/register").await,
            &[
                CHROME[0],
                "action=\"/auth/register\"",
                "Password Confirmation",
            ],
        );
        assert_shell(
            &server.get("/logout").await,
            &[CHROME[0], "action=\"/auth/logout\""],
        );

        // A guest who visits a guarded page gets an inline router redirect:
        // the SSR body is already the login form (a component-level redirect
        // can't issue a 303 over plain HTTP; the POST handlers below do).
        for path in ["/profile", "/profile/change-password"] {
            let response = server.get(path).await;
            assert_shell(&response, &[CHROME[0], "action=\"/auth/login\""]);
        }
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn login_form_sets_the_session_and_lands_home(pool: PgPool) {
        // The fixture hashes the word "password" as test-5.
        create_test_user(&pool, 5).await.expect("user fixture");
        let server = api_server(pool).await;

        let response = post_form(
            &server,
            "/auth/login",
            &[
                ("username", "test-5"),
                ("password", "password"),
                ("remember", "on"),
            ],
        )
        .await;
        response.assert_status(StatusCode::SEE_OTHER);
        assert_eq!(location(&response), "/");

        // The home page's session read shows the signed-in menu.
        let home = server.get("/").await.text();
        assert!(home.contains("test-5"), "signed-in navbar:\n{home}");
        assert!(home.contains("Log out"), "signed-in navbar:\n{home}");
        assert!(!home.contains("Register"), "guest links gone:\n{home}");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn login_form_notices_for_bad_attempts(pool: PgPool) {
        create_test_user(&pool, 5).await.expect("user fixture");
        let server = api_server(pool).await;

        let wrong_password = post_form(
            &server,
            "/auth/login",
            &[("username", "test-5"), ("password", "nope")],
        )
        .await;
        assert_eq!(location(&wrong_password), "/login?bad=password");

        let unknown_user = post_form(
            &server,
            "/auth/login",
            &[("username", "nobody"), ("password", "password")],
        )
        .await;
        assert_eq!(location(&unknown_user), "/login?bad=not_found");

        let empty_fields = post_form(
            &server,
            "/auth/login",
            &[("username", ""), ("password", "")],
        )
        .await;
        assert_eq!(location(&empty_fields), "/login?bad=required");

        // The page turns a code back into the Leptos alert text.
        let page = server.get("/login?bad=password").await;
        assert_shell(
            &page,
            &[CHROME[0], "alert alert-error", "Password does not match."],
        );
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn logout_form_ends_the_session(pool: PgPool) {
        create_test_user(&pool, 5).await.expect("user fixture");
        let server = api_server(pool).await;

        post_form(
            &server,
            "/auth/login",
            &[("username", "test-5"), ("password", "password")],
        )
        .await
        .assert_status(StatusCode::SEE_OTHER);

        let before = server.get("/logout").await.text();
        assert!(before.contains("action=\"/auth/logout\""), "{before}");

        let after = post_form(&server, "/auth/logout", &[]).await;
        assert_eq!(location(&after), "/logout?done=1");

        // The done state shows the Leptos page's "Logged out" line, and the
        // home page is back to the guest menu.
        let done_page = server.get("/logout?done=1").await;
        assert_shell(&done_page, &[CHROME[0], "Logged out"]);
        let home = server.get("/").await.text();
        assert!(home.contains("Register"), "session should be gone:\n{home}");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn register_form_signs_the_new_user_in(pool: PgPool) {
        let server = api_server(pool).await;

        let created = post_form(
            &server,
            "/auth/register",
            &[
                ("username", "new-person"),
                ("email", "new@example.com"),
                ("password", "secret123"),
                ("password_confirmation", "secret123"),
            ],
        )
        .await;
        assert_eq!(location(&created), "/");
        let home = server.get("/").await.text();
        assert!(home.contains("new-person"), "signed-in navbar:\n{home}");

        let duplicate = post_form(
            &server,
            "/auth/register",
            &[
                ("username", "new-person"),
                ("email", "other@example.com"),
                ("password", "secret123"),
                ("password_confirmation", "secret123"),
            ],
        )
        .await;
        assert_eq!(location(&duplicate), "/register?bad=taken");

        let mismatch = post_form(
            &server,
            "/auth/register",
            &[
                ("username", "third-person"),
                ("email", "third@example.com"),
                ("password", "secret123"),
                ("password_confirmation", "other456"),
            ],
        )
        .await;
        assert_eq!(location(&mismatch), "/register?bad=mismatch");
        let page = server.get("/register?bad=mismatch").await;
        assert_shell(&page, &[CHROME[0], "Passwords did not match."]);
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn profile_and_password_forms_redirect_guests(pool: PgPool) {
        let server = api_server(pool).await;

        let profile = post_form(
            &server,
            "/auth/profile",
            &[
                ("original_username", "test-5"),
                ("username", "hacker"),
                ("email", "hacker@example.com"),
            ],
        )
        .await;
        assert_eq!(location(&profile), "/login");

        let password = post_form(
            &server,
            "/auth/change-password",
            &[
                ("password", "x"),
                ("new_password", "y"),
                ("new_password_confirmation", "y"),
            ],
        )
        .await;
        assert_eq!(location(&password), "/login");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn profile_form_saves_the_session_user(pool: PgPool) {
        create_test_user(&pool, 5).await.expect("user fixture");
        let server = api_server(pool).await;
        post_form(
            &server,
            "/auth/login",
            &[("username", "test-5"), ("password", "password")],
        )
        .await
        .assert_status(StatusCode::SEE_OTHER);

        let saved = post_form(
            &server,
            "/auth/profile",
            &[
                ("original_username", "test-5"),
                ("username", "test-5"),
                ("email", "test-5@example.com"),
                ("first_name", "Testy"),
                ("last_name", ""),
                ("description", "hello"),
            ],
        )
        .await;
        assert_eq!(location(&saved), "/profile?saved=1");

        // The reload re-fills the form with the stored values, and the
        // `?saved` code raises the Leptos page's success alert.
        let page = server.get("/profile").await.text();
        assert!(page.contains("Testy"), "reloaded profile:\n{page}");
        let alerted = server.get("/profile?saved=1").await.text();
        assert!(
            alerted.contains("alert alert-success") && alerted.contains("test-5 Updated!"),
            "success alert:\n{alerted}"
        );

        // A signed-in session still cannot edit someone else's account.
        let impersonate = post_form(
            &server,
            "/auth/profile",
            &[
                ("original_username", "someone-else"),
                ("username", "whoever"),
                ("email", "whoever@example.com"),
            ],
        )
        .await;
        assert_eq!(location(&impersonate), "/profile?bad=forbidden");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn change_password_form_updates_the_password(pool: PgPool) {
        create_test_user(&pool, 5).await.expect("user fixture");
        let server = api_server(pool).await;
        post_form(
            &server,
            "/auth/login",
            &[("username", "test-5"), ("password", "password")],
        )
        .await
        .assert_status(StatusCode::SEE_OTHER);

        let wrong_current = post_form(
            &server,
            "/auth/change-password",
            &[
                ("password", "nope"),
                ("new_password", "fresh123"),
                ("new_password_confirmation", "fresh123"),
            ],
        )
        .await;
        assert_eq!(
            location(&wrong_current),
            "/profile/change-password?bad=current"
        );

        let changed = post_form(
            &server,
            "/auth/change-password",
            &[
                ("password", "password"),
                ("new_password", "fresh123"),
                ("new_password_confirmation", "fresh123"),
            ],
        )
        .await;
        assert_eq!(location(&changed), "/profile/change-password?updated=1");
        let alerted = server
            .get("/profile/change-password?updated=1")
            .await
            .text();
        assert!(
            alerted.contains("alert alert-success") && alerted.contains("Password Updated!"),
            "success alert:\n{alerted}"
        );

        // The old password is dead and the new one logs in.
        let stale = post_form(
            &server,
            "/auth/login",
            &[("username", "test-5"), ("password", "password")],
        )
        .await;
        assert_eq!(location(&stale), "/login?bad=password");
        let fresh = post_form(
            &server,
            "/auth/login",
            &[("username", "test-5"), ("password", "fresh123")],
        )
        .await;
        assert_eq!(location(&fresh), "/");
    }

    // --- Admin pages and forms (MIGRATION.md Phase 4 slice 4) ----------

    /// Log `test-<n>` (password "password") into an `api_server` jar.
    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    async fn login_as(server: &axum_test::TestServer, n: u32) {
        post_args(
            server,
            "/api/login",
            &LoginArgs {
                username: &format!("test-{n}"),
                password: "password",
            },
        )
        .await
        .assert_status_ok();
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn signed_in_non_admins_get_the_login_form(pool: PgPool) {
        create_test_record_label(&pool, 1).await.expect("label");
        create_test_user_with_permissions(&pool, 1, vec!["user"])
            .await
            .expect("non-admin user");

        let server = api_server(pool).await;
        login_as(&server, 1).await;

        let response = server.get("/admin").await;
        assert_shell(
            &response,
            &[CHROME[0], "<h1>Login</h1>", "action=\"/auth/login\""],
        );
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn dashboard_renders_cards_and_menu_for_an_owner(pool: PgPool) {
        let label = create_test_record_label(&pool, 1).await.expect("label");
        let artist = create_test_artist(&pool, 1, Some(label.clone()))
            .await
            .expect("artist");
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .expect("release");
        create_test_page(&pool, 1, Some(label.clone()))
            .await
            .expect("page");
        // Move the release into the future so the countdown card renders.
        sqlx::query("UPDATE releases SET release_date = now() + interval '3 days' WHERE id = $1")
            .bind(release.id)
            .execute(&pool)
            .await
            .expect("future release");
        create_test_user_with_permissions(&pool, 1, vec!["admin", "label_owner"])
            .await
            .expect("admin+label_owner user");

        let server = api_server(pool).await;
        login_as(&server, 1).await;

        let body = server.get("/admin").await.text();
        for marker in [
            CHROME[0],
            "<title>Admin Dashboard</title>",
            // Card headings.
            "card-title\">Artists",
            "card-title\">Pages",
            "Social Tips!",
            "Mailing list",
            // Seeded rows.
            "Test Artist 1",
            "Test Page 1",
            // The label card and its Edit link (label_owner sessions only).
            "Test Record Label 1",
            "href=\"/admin/label\"",
            // The upcoming-release countdown card (values are daisy CSS
            // custom properties).
            "Upcoming Release — Test Release 1",
            "--value:",
            // The side menu: label, artist submenu and page link.
            "w-56 menu bg-base-200 rounded-box",
            "href=\"/admin\"",
            "href=\"/admin/artist/test-artist-1\"",
            "All Releases",
            "Test Release 1",
            "href=\"/admin/page/test-page-1\"",
            "Create Artist",
            "Create Page",
        ] {
            assert!(
                body.contains(marker),
                "SSR body missing `{marker}`:\n{body}"
            );
        }
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn no_upcoming_release_hides_the_countdown(pool: PgPool) {
        let label = create_test_record_label(&pool, 1).await.expect("label");
        let artist = create_test_artist(&pool, 1, Some(label))
            .await
            .expect("artist");
        create_test_release(&pool, 1, Some(artist))
            .await
            .expect("past-dated release");
        create_test_user_with_permissions(&pool, 1, vec!["admin"])
            .await
            .expect("admin user");

        let server = api_server(pool).await;
        login_as(&server, 1).await;

        let body = server.get("/admin").await.text();
        assert!(body.contains("card-title\">Artists"), "{body}");
        assert!(
            !body.contains("Upcoming Release"),
            "a past release should not render the countdown:\n{body}"
        );
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn guest_label_and_page_forms_redirect_to_login(pool: PgPool) {
        let server = test_server(pool, None).await;

        let paths = [
            "/admin/forms/label",
            "/admin/forms/page",
            "/admin/forms/page/update",
            "/admin/forms/page/delete",
            "/admin/forms/page/restore",
        ];
        for path in paths {
            let response = post_form(&server, path, &[]).await;
            assert_eq!(
                response.status_code(),
                axum::http::StatusCode::SEE_OTHER,
                "{path} should 303"
            );
            assert_eq!(location(&response), "/login", "{path} guest target");
        }
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn label_form_round_trip_for_a_label_owner(pool: PgPool) {
        let label = create_test_record_label(&pool, 1).await.expect("label");
        create_test_user_with_permissions(&pool, 1, vec!["admin", "label_owner"])
            .await
            .expect("admin+label_owner user");

        let server = api_server(pool).await;
        login_as(&server, 1).await;

        let response = post_form(
            &server,
            "/admin/forms/label",
            &[
                ("id", &label.id.to_string()),
                ("name", "Renamed Label"),
                ("description", "new story"),
                ("isrc_base", "US ABC 1"),
            ],
        )
        .await;
        assert_eq!(response.status_code(), axum::http::StatusCode::SEE_OTHER);
        assert_eq!(location(&response), "/admin/label?updated=1");

        // The reload shows the saved values, the success alert, and the
        // ISRC explainer.
        let body = server.get("/admin/label?updated=1").await.text();
        for marker in [
            CHROME[0],
            "Edit Renamed Label Details",
            "Renamed Label Updated!",
            "value=\"Renamed Label\"",
            "What is an ISRC code?",
        ] {
            assert!(
                body.contains(marker),
                "SSR body missing `{marker}`:\n{body}"
            );
        }
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn label_form_is_forbidden_without_label_owner(pool: PgPool) {
        let label = create_test_record_label(&pool, 1).await.expect("label");
        // `admin` alone is not enough; the check is `label_owner`.
        create_test_user_with_permissions(&pool, 1, vec!["admin"])
            .await
            .expect("admin-only user");

        let server = api_server(pool).await;
        login_as(&server, 1).await;

        let response = post_form(
            &server,
            "/admin/forms/label",
            &[("id", &label.id.to_string()), ("name", "Hijacked Label")],
        )
        .await;
        assert_eq!(response.status_code(), axum::http::StatusCode::SEE_OTHER);
        assert_eq!(location(&response), "/admin/label?bad=forbidden");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn page_form_flows_for_a_label_owner(pool: PgPool) {
        let label = create_test_record_label(&pool, 1).await.expect("label");
        create_test_user_with_permissions(&pool, 1, vec!["admin", "label_owner"])
            .await
            .expect("admin+label_owner user");

        let server = api_server(pool).await;
        login_as(&server, 1).await;
        let label_id = label.id.to_string();

        // Create: the 303 target is the editor under the server-derived
        // slug; the page then renders its editor form.
        let response = post_form(
            &server,
            "/admin/forms/page",
            &[
                ("label_id", label_id.as_str()),
                ("name", "My New Page"),
                ("description", "meta"),
                ("body", "# hi"),
                ("local_published_at", "2026-03-01T09:30"),
            ],
        )
        .await;
        assert_eq!(response.status_code(), axum::http::StatusCode::SEE_OTHER);
        assert_eq!(location(&response), "/admin/page/my-new-page");

        let body = server.get("/admin/page/my-new-page").await.text();
        for marker in [
            CHROME[0],
            "Edit My New Page",
            "action=\"/admin/forms/page/update\"",
            "value=\"2026-03-01T09:30\"", // datetime-local value
            "Timezone +0000 (UTC)",
        ] {
            assert!(
                body.contains(marker),
                "SSR body missing `{marker}`:\n{body}"
            );
        }

        // Update with a rename: `update` re-slugifies, and the redirect
        // lands on the new slug carrying ?updated=1.
        let response = post_form(
            &server,
            "/admin/forms/page/update",
            &[
                ("slug", "my-new-page"),
                ("name", "Renamed Page"),
                ("description", "meta"),
                ("body", "# hi"),
            ],
        )
        .await;
        assert_eq!(response.status_code(), axum::http::StatusCode::SEE_OTHER);
        assert_eq!(location(&response), "/admin/page/renamed-page?updated=1");

        let body = server
            .get("/admin/page/renamed-page?updated=1")
            .await
            .text();
        assert!(body.contains("Renamed Page Updated!"), "{body}");
        assert!(body.contains("id=\"page-delete\""), "{body}");

        // Delete swaps the row's Restore control in, then restore swaps
        // back — both round-trip through 303 + reload.
        let response = post_form(
            &server,
            "/admin/forms/page/delete",
            &[("slug", "renamed-page")],
        )
        .await;
        assert_eq!(location(&response), "/admin/page/renamed-page?updated=1");
        let body = server
            .get("/admin/page/renamed-page?updated=1")
            .await
            .text();
        assert!(body.contains("id=\"page-restore\""), "{body}");

        let response = post_form(
            &server,
            "/admin/forms/page/restore",
            &[("slug", "renamed-page")],
        )
        .await;
        assert_eq!(location(&response), "/admin/page/renamed-page?updated=1");
        let body = server
            .get("/admin/page/renamed-page?updated=1")
            .await
            .text();
        assert!(body.contains("id=\"page-delete\""), "{body}");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn page_form_failures_carry_fixed_codes(pool: PgPool) {
        let label = create_test_record_label(&pool, 1).await.expect("label");
        create_test_user_with_permissions(&pool, 1, vec!["admin", "label_owner"])
            .await
            .expect("admin+label_owner user");
        create_test_page(&pool, 1, Some(label.clone()))
            .await
            .expect("page");

        let server = api_server(pool).await;
        login_as(&server, 1).await;
        let label_id = label.id.to_string();

        // Empty name: the validation message's fixed code.
        let response = post_form(
            &server,
            "/admin/forms/page",
            &[("label_id", label_id.as_str()), ("name", "")],
        )
        .await;
        assert_eq!(location(&response), "/admin/page?bad=name_required");
        let body = server.get(&location(&response)).await.text();
        assert!(body.contains("Name is required."), "{body}");
        assert!(
            !body.contains("name_required<"),
            "the code itself must not be echoed:\n{body}"
        );

        // A new page whose name slugifies onto an existing slug
        // (test-page-1): the uniqueness validation's fixed code.
        let response = post_form(
            &server,
            "/admin/forms/page",
            &[("label_id", label_id.as_str()), ("name", "Test Page 1")],
        )
        .await;
        assert_eq!(location(&response), "/admin/page?bad=slug_taken");

        // Deleting an unknown page: the generic code. (The EditPage GET at
        // that slug falls through to the 404 inline — the alert never
        // renders there, so only the redirect target is asserted.)
        let response = post_form(
            &server,
            "/admin/forms/page/delete",
            &[("slug", "no-such-page")],
        )
        .await;
        assert_eq!(location(&response), "/admin/page/no-such-page?bad=server");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn page_create_is_forbidden_for_plain_admins(pool: PgPool) {
        let label = create_test_record_label(&pool, 1).await.expect("label");
        create_test_user_with_permissions(&pool, 1, vec!["admin"])
            .await
            .expect("admin-only user");

        let server = api_server(pool).await;
        login_as(&server, 1).await;
        let label_id = label.id.to_string();

        let response = post_form(
            &server,
            "/admin/forms/page",
            &[("label_id", label_id.as_str()), ("name", "Nope")],
        )
        .await;
        assert_eq!(location(&response), "/admin/page?bad=forbidden");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn nested_admin_layouts_render_outside_in(pool: PgPool) {
        // Slice 4 gates the layout, so the nested-route checks now run with
        // an admin session (the tab layout itself is still the placeholder
        // until slice 5).
        create_test_user_with_permissions(&pool, 1, vec!["admin"])
            .await
            .expect("admin user");
        let server = api_server(pool).await;
        login_as(&server, 1).await;

        let response = server.get("/admin/artist/jimi").await;

        // AppShell chrome, AdminRoot menu + article wrapper,
        // AdminArtistRoot tabs, and the page itself — with the slug prop
        // reaching the child component.
        assert_shell(
            &response,
            &[
                CHROME[0],
                CHROME[1],
                "w-56 menu bg-base-200 rounded-box",
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
        create_test_user_with_permissions(&pool, 1, vec!["admin"])
            .await
            .expect("admin user");
        let server = api_server(pool).await;
        login_as(&server, 1).await;

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

    #[test]
    fn uploads_mount_decisions_cover_skip_conflict_and_mount() {
        let root = tempfile::tempdir().expect("fixture root");
        let public = root.path().join("public");
        std::fs::create_dir_all(public.join("uploads")).expect("public uploads entry");
        let outside = root.path().join("uploads");
        std::fs::create_dir_all(&outside).expect("outside uploads dir");

        // Skip: the uploads root *is* the public dir's `uploads` entry.
        assert_eq!(
            uploads_mount(&public.join("uploads"), Some(&public)),
            UploadsMount::Skip
        );
        // Conflict: a public `uploads` entry blocks a different uploads root.
        assert_eq!(
            uploads_mount(&outside, Some(&public)),
            UploadsMount::Conflict
        );
        // Mount: no public dir, or no `uploads` entry in it.
        assert_eq!(uploads_mount(&outside, None), UploadsMount::Mount);
        assert_eq!(
            uploads_mount(&outside, Some(&root.path().join("missing"))),
            UploadsMount::Mount
        );
    }
}
