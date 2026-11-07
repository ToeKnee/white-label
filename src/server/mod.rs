//! Dioxus server side of the migration (see MIGRATION.md, Phases 1–3).
//!
//! Assembles the axum router that `dioxus::serve` wraps: the app database
//! pool as a request extension, `tower-sessions` backed by Postgres,
//! static uploads, the auth form handlers (`forms`) and the Dioxus SSR app
//! (`crate::ui`), whose server fns carry the JSON API.

pub mod current_user;
pub(crate) mod forms;
#[cfg(test)]
pub(crate) mod test_support;
pub(crate) mod upload_form;

use anyhow::Context;
use axum::Router;
use tower_sessions::SessionManagerLayer;
use tower_sessions_sqlx_store::PostgresStore;

use crate::server::current_user::Db;

/// Postgres table holding this server's sessions.
///
/// Named `sessions` (the `axum_sessions` table the pre-Dioxus server used is
/// no longer read by any code; the schema itself is left alone).
const SESSION_TABLE: &str = "sessions";

/// Uploads directory (`UPLOAD_PATH`), exposed to API handlers as a request
/// extension.
///
/// `None` when the server has no uploads dir. Passing it through the router
/// keeps the value injectable for tests.
#[derive(Clone, Debug)]
pub struct UploadRoot(Option<std::path::PathBuf>);

impl UploadRoot {
    /// Wrap the uploads directory (if the server has one).
    #[must_use]
    pub const fn new(path: Option<std::path::PathBuf>) -> Self {
        Self(path)
    }

    /// The root as a path string, for `{root}/{sub}/{file}` joins — the shape
    /// the upload paths stored in the database are written against.
    #[must_use]
    pub fn root(&self) -> Option<String> {
        self.0.as_ref().map(|path| path.display().to_string())
    }
}

/// Build the Dioxus server's router (database, sessions, uploads, Dioxus
/// SSR). Factored out from serving so `axum-test` can drive it without
/// binding a socket.
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
        // The auth pages' form posts (see `forms`), POST counterparts of
        // the SSR pages' paths, also under the session and `Db` layers.
        .merge(forms::form_routes())
        .layer(SessionManagerLayer::new(session_store))
        // Handlers and server fns reach the pool via `Db`.
        .layer(axum::Extension(Db(pool)))
        // The upload paths (the `ui::upload` server fn and the no-JS
        // handler in `server::upload_form`) resolve their root here.
        .layer(axum::Extension(UploadRoot::new(upload_path.clone())));

    // Uploaded files, mounted after the session stack so static hits skip
    // the database.
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
    use crate::server::build_router;
    use crate::server::test_support::{self, api_server};

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
        // (the fetches fail), so
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
        // render, so the admin menu never appears). The seeded artist
        // profile (slice 5a) is asserted with an admin session below, as
        // are the placeholder release/track pages.
        let routes = [
            "/admin",
            "/admin/label",
            "/admin/artist",
            "/admin/artist/jimi",
            "/admin/page",
            "/admin/page/terms-of-use",
            "/admin/artist/jimi/releases",
            "/admin/artist/jimi/releases/new",
            "/admin/artist/jimi/release/axis",
            "/admin/artist/jimi/release/axis/tracks",
            "/admin/artist/jimi/release/axis/tracks/new",
            "/admin/artist/jimi/release/axis/track/castles",
            "/admin/artist/jimi/links",
            "/admin/artist/jimi/images",
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

        login_as(&server, 2).await;

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

    /// Repeat-capable sibling of `post_form`: the release "All Artists"
    /// checkboxes post the same `artist_ids` key once per checked artist.
    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; these tests run on one thread via block_on, so Send isn't needed"
    )]
    async fn post_form_pairs(
        server: &TestServer,
        path: &str,
        fields: &[(&str, &str)],
    ) -> TestResponse {
        server.post(path).form(&fields.to_vec()).await
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

        // The page turns a code back into the alert text.
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

        // The done state shows the "Logged out" line, and the
        // home page is back to the guest menu.
        let done_page = server.get("/logout?done=1").await;
        assert_shell(&done_page, &[CHROME[0], "Logged out"]);
        let home = server.get("/").await.text();
        assert!(home.contains("Register"), "session should be gone:\n{home}");
    }

    // --- Reactive auth twins (`POST /api/login` & co, Phase 6 slice 6d) ----
    //
    // The desktop window and the wasm client cannot post a form and follow the
    // 303, so these JSON twins carry the same flows as data. They share the
    // services and the session records with the handlers above, so the tests
    // check the outcome shape and that the session really is shared.

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn login_twin_answers_with_the_user_and_starts_the_session(pool: PgPool) {
        create_test_user(&pool, 5).await.expect("user fixture");
        let server = api_server(pool).await;

        let answered = server
            .post("/api/login")
            .json(&serde_json::json!({
                "form": { "username": "test-5", "password": "password", "remember": "on" }
            }))
            .await;
        answered.assert_status_ok();
        let body = answered.text();
        assert!(
            body.contains("LoggedIn"),
            "outcome should be a session:\n{body}"
        );
        assert!(body.contains("test-5"), "and carry the user:\n{body}");

        // The session cookie that answer set is the one every other twin reads:
        // the JSON session endpoint and the SSR chrome's menu both see it.
        let user = server.get("/api/user").await.text();
        assert!(
            user.contains("test-5"),
            "the session should be live:\n{user}"
        );
        let home = server.get("/").await.text();
        assert!(home.contains("Log out"), "signed-in navbar:\n{home}");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn login_twin_reports_the_forms_fixed_codes(pool: PgPool) {
        create_test_user(&pool, 5).await.expect("user fixture");
        let server = api_server(pool).await;

        // A refused login is still a plain answer: the client maps the code
        // through the same alert table the `?bad=` redirect path uses.
        for (username, password, code) in [
            ("test-5", "nope", "password"),
            ("nobody", "password", "not_found"),
            ("", "", "required"),
        ] {
            let answered = server
                .post("/api/login")
                .json(&serde_json::json!({
                    "form": { "username": username, "password": password }
                }))
                .await;
            answered.assert_status_ok();
            let body = answered.text();
            assert!(
                body.contains("Failed") && body.contains(code),
                "expected {code} for {username:?}, got:\n{body}"
            );
        }

        // Nothing was signed in along the way.
        let user = server.get("/api/user").await.text();
        assert_eq!(user.trim(), "null", "no session should exist:\n{user}");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn logout_twin_ends_the_session(pool: PgPool) {
        create_test_user(&pool, 5).await.expect("user fixture");
        let server = api_server(pool).await;

        server
            .post("/api/login")
            .json(&serde_json::json!({
                "form": { "username": "test-5", "password": "password" }
            }))
            .await
            .assert_status_ok();
        assert!(
            server.get("/api/user").await.text().contains("test-5"),
            "the login twin should have started a session"
        );

        // A unit variant arrives as the bare string "Done".
        let ended = server
            .post("/api/logout")
            .json(&serde_json::json!({}))
            .await;
        ended.assert_status_ok();
        assert_eq!(ended.text().trim(), "\"Done\"", "logout outcome");

        let user = server.get("/api/user").await.text();
        assert_eq!(user.trim(), "null", "session should be gone:\n{user}");
        let home = server.get("/").await.text();
        assert!(
            home.contains("Register"),
            "guest menu should be back:\n{home}"
        );
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn register_twin_creates_the_user_and_signs_them_in(pool: PgPool) {
        let server = api_server(pool).await;

        let answered = server
            .post("/api/register")
            .json(&serde_json::json!({
                "form": {
                    "username": "twin-user",
                    "email": "twin-user@example.com",
                    "password": "password",
                    "password_confirmation": "password",
                    "remember": "on",
                }
            }))
            .await;
        answered.assert_status_ok();
        let body = answered.text();
        assert!(
            body.contains("LoggedIn"),
            "register logs the new user in:\n{body}"
        );
        assert!(body.contains("twin-user"), "and returns them:\n{body}");
        assert!(
            server.get("/api/user").await.text().contains("twin-user"),
            "the session should belong to the new user"
        );

        // The same fixed codes the register form reports go back out.
        let duplicate = server
            .post("/api/register")
            .json(&serde_json::json!({
                "form": {
                    "username": "twin-user",
                    "email": "twin-user@example.com",
                    "password": "password",
                    "password_confirmation": "password",
                }
            }))
            .await;
        duplicate.assert_status_ok();
        let body = duplicate.text();
        assert!(
            body.contains("Failed") && body.contains("taken"),
            "the taken code should come back:\n{body}"
        );

        // Mismatched confirmations report `mismatch`.
        let mismatch = server
            .post("/api/register")
            .json(&serde_json::json!({
                "form": {
                    "username": "another-user",
                    "email": "another-user@example.com",
                    "password": "password",
                    "password_confirmation": "nope",
                }
            }))
            .await;
        mismatch.assert_status_ok();
        let body = mismatch.text();
        assert!(
            body.contains("Failed") && body.contains("mismatch"),
            "the mismatch code should come back:\n{body}"
        );
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
        // `?saved` code raises the success alert.
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
        // The three fields share a key icon, so their placeholders are what
        // tells the user which is which.
        for marker in [
            "placeholder=\"Current Password\"",
            "placeholder=\"New Password\"",
            "placeholder=\"New Password Confirmation\"",
        ] {
            assert!(
                alerted.contains(marker),
                "SSR body missing `{marker}`:\n{alerted}"
            );
        }

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

    // --- Reactive profile and change-password twins (Phase 6 slice 6i) ---
    //
    // A webview window cannot follow the `/profile?saved=1` and
    // `?bad=<code>` redirects these handlers answer with, so the twins carry the
    // same outcome as data: the account as the server now holds it (which the
    // form re-fills and the navbar shows), or the handler's own code.

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn profile_twin_answers_the_stored_account(pool: PgPool) {
        create_test_user(&pool, 5).await.expect("user fixture");
        let server = api_server(pool).await;
        server
            .post("/api/login")
            .json(&serde_json::json!({
                "form": { "username": "test-5", "password": "password" }
            }))
            .await
            .assert_status_ok();

        // A rename answers the stored account, which is what re-fills the form's
        // fields and reaches the navbar.
        let answer = server
            .post("/api/update_profile")
            .json(&serde_json::json!({
                "fields": {
                    "original_username": "test-5",
                    "username": "test-five",
                    "email": "test-5@example.com",
                    "first_name": "Testy",
                    "last_name": "",
                    "description": "",
                }
            }))
            .await;
        answer.assert_status_ok();
        let saved: serde_json::Value = answer.json();
        assert_eq!(
            saved["Saved"]["username"].as_str(),
            Some("test-five"),
            "the saved account comes back:\n{saved}"
        );

        // The store really holds the rename, and a blank input cleared a value
        // rather than storing "" (the same mapping the javascript-off handler
        // applies).
        let session = server.get("/api/user").await;
        let stored: serde_json::Value = session.json();
        assert!(
            stored.is_object(),
            "the session read should answer the stored user:\n{stored}"
        );
        assert_eq!(
            stored["username"].as_str(),
            Some("test-five"),
            "the rename should be stored:\n{stored}"
        );
        assert_eq!(
            stored["first_name"].as_str(),
            Some("Testy"),
            "a filled input should be stored:\n{stored}"
        );
        // A blank input cleared the value rather than storing "" (the same
        // mapping the javascript-off handler applies).
        assert!(
            stored["last_name"].is_null(),
            "a blank last name should be no value:\n{stored}"
        );
        assert!(
            stored["description"].is_null(),
            "a blank description should be no value:\n{stored}"
        );

        // Editing someone else's account is refused with the handler's code.
        let answer = server
            .post("/api/update_profile")
            .json(&serde_json::json!({
                "fields": {
                    "original_username": "someone-else",
                    "username": "whoever",
                    "email": "whoever@example.com",
                }
            }))
            .await;
        answer.assert_status_ok();
        let refused: serde_json::Value = answer.json();
        assert_eq!(
            refused["Failed"]["code"].as_str(),
            Some("forbidden"),
            "the forbidden code should come back:\n{refused}"
        );
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn change_password_twin_answers_done_and_reuses_the_codes(pool: PgPool) {
        create_test_user(&pool, 5).await.expect("user fixture");
        let server = api_server(pool).await;
        server
            .post("/api/login")
            .json(&serde_json::json!({
                "form": { "username": "test-5", "password": "password" }
            }))
            .await
            .assert_status_ok();

        // The current password has to be right, and the refusal carries the
        // `current` code the redirect would have carried.
        let answer = server
            .post("/api/change_password")
            .json(&serde_json::json!({
                "fields": {
                    "password": "nope",
                    "new_password": "fresh123",
                    "new_password_confirmation": "fresh123",
                }
            }))
            .await;
        answer.assert_status_ok();
        let refused: serde_json::Value = answer.json();
        assert_eq!(
            refused["Failed"]["code"].as_str(),
            Some("current"),
            "the wrong current password should report `current`:\n{refused}"
        );

        // A confirmation mismatch reports `mismatch`.
        let answer = server
            .post("/api/change_password")
            .json(&serde_json::json!({
                "fields": {
                    "password": "password",
                    "new_password": "fresh123",
                    "new_password_confirmation": "other",
                }
            }))
            .await;
        answer.assert_status_ok();
        let refused: serde_json::Value = answer.json();
        assert_eq!(
            refused["Failed"]["code"].as_str(),
            Some("mismatch"),
            "the mismatch code should come back:\n{refused}"
        );

        // A finished change answers `Done` — the page's success alert, with no
        // row to hand back because nothing about the account changed.
        let answer = server
            .post("/api/change_password")
            .json(&serde_json::json!({
                "fields": {
                    "password": "password",
                    "new_password": "fresh123",
                    "new_password_confirmation": "fresh123",
                }
            }))
            .await;
        answer.assert_status_ok();
        assert_eq!(answer.text().trim(), "\"Done\"", "the change outcome");

        // The new password is really the one on the account: the login twin
        // refuses the old one and takes the new one.
        let answer = server
            .post("/api/login")
            .json(&serde_json::json!({
                "form": { "username": "test-5", "password": "password" }
            }))
            .await;
        let refused: serde_json::Value = answer.json();
        assert_eq!(
            refused["Failed"]["code"].as_str(),
            Some("password"),
            "the old password should be dead:\n{refused}"
        );

        let answer = server
            .post("/api/login")
            .json(&serde_json::json!({
                "form": { "username": "test-5", "password": "fresh123" }
            }))
            .await;
        let signed_in: serde_json::Value = answer.json();
        assert_eq!(
            signed_in["LoggedIn"]["user"]["username"].as_str(),
            Some("test-5"),
            "the new password should log in:\n{signed_in}"
        );
    }

    // --- Admin pages and forms (MIGRATION.md Phase 4 slice 4) ----------

    /// Log `test-<n>` (password "password") into an `api_server` jar, through
    /// the login server fn the Dioxus clients use.
    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    async fn login_as(server: &axum_test::TestServer, n: u32) {
        test_support::login_as(server, &format!("test-{n}")).await;
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
            "href=\"/admin/label?\"",
            // The upcoming-release countdown card (values are daisy CSS
            // custom properties).
            "Upcoming Release — Test Release 1",
            "--value:",
            // The side menu: label, artist submenu and page link.
            "w-56 menu bg-base-200 rounded-box",
            "href=\"/admin\"",
            "href=\"/admin/artist/test-artist-1/?\"",
            "All Releases",
            "Test Release 1",
            "href=\"/admin/page/test-page-1?\"",
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
            "/admin/forms/artist",
            "/admin/forms/artist/update",
            "/admin/forms/artist/delete",
            "/admin/forms/artist/restore",
            "/admin/forms/release",
            "/admin/forms/release/update",
            "/admin/forms/release/delete",
            "/admin/forms/release/restore",
            "/admin/forms/track",
            "/admin/forms/track/update",
            "/admin/forms/track/delete",
            "/admin/forms/track/restore",
            "/admin/forms/links",
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

    // --- Artist profile forms (MIGRATION.md Phase 4 slice 5a) ----------

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn artist_profile_flows_for_a_label_owner(pool: PgPool) {
        create_test_record_label(&pool, 1).await.expect("label");
        create_test_user_with_permissions(&pool, 1, vec!["admin", "label_owner"])
            .await
            .expect("admin + label_owner user");

        let server = api_server(pool).await;
        login_as(&server, 1).await;

        // The create page renders the empty form with the label stamped in.
        let create_page = server.get("/admin/artist").await;
        assert_shell(
            &create_page,
            &[
                "Create Artist",
                "action=\"/admin/forms/artist\"",
                "name=\"label_id\" value=\"1\"",
            ],
        );

        // Create 303s into the editor under the slug derived from the name —
        // silently, with no `?updated=1`.
        let created = post_form(
            &server,
            "/admin/forms/artist",
            &[
                ("label_id", "1"),
                ("name", "Jimi Solo"),
                ("description", "Playing around."),
                ("website", "https://jimi.example.com"),
                ("local_published_at", "2026-03-01T09:30"),
            ],
        )
        .await;
        assert_eq!(location(&created), "/admin/artist/jimi-solo");

        // The profile editor shows the stored values inside the tab layout.
        let editor = server.get("/admin/artist/jimi-solo").await;
        assert_shell(
            &editor,
            &[
                "Jimi Solo Profile",
                "action=\"/admin/forms/artist/update\"",
                "value=\"Jimi Solo\"",
                "value=\"2026-03-01T09:30\"",
                "Timezone +0000 (UTC)",
                "tab tab-active",
            ],
        );

        // Renaming re-slugifies (like `Page::update` did in slice 4): the
        // redirect carries the *new* slug with the success code.
        let renamed = post_form(
            &server,
            "/admin/forms/artist/update",
            &[
                ("slug", "jimi-solo"),
                ("name", "Renamed Artist"),
                ("description", "Playing around."),
                ("website", "https://jimi.example.com"),
                ("local_published_at", "2026-03-01T09:30"),
            ],
        )
        .await;
        assert_eq!(location(&renamed), "/admin/artist/renamed-artist?updated=1");
        // GET the redirect target as a browser would, query included, so
        // the success alert renders.
        let editor = server.get("/admin/artist/renamed-artist?updated=1").await;
        assert_shell(
            &editor,
            &[
                "Renamed Artist Profile",
                "Renamed Artist Updated!",
                "value=\"Renamed Artist\"",
            ],
        );

        // Delete swaps the row button to Restore (the JS-free reload flow).
        // `get_artist` answers by slug
        // without a visibility filter, so the soft-deleted row still loads.
        let deleted = post_form(
            &server,
            "/admin/forms/artist/delete",
            &[("slug", "renamed-artist")],
        )
        .await;
        assert_eq!(location(&deleted), "/admin/artist/renamed-artist?updated=1");
        let body = server.get("/admin/artist/renamed-artist").await.text();
        assert!(
            body.contains("btn btn-secondary") && !body.contains("btn btn-error"),
            "the deleted artist should show Restore, not Delete:\n{body}"
        );

        // Restore swaps the row button back.
        let restored = post_form(
            &server,
            "/admin/forms/artist/restore",
            &[("slug", "renamed-artist")],
        )
        .await;
        assert_eq!(
            location(&restored),
            "/admin/artist/renamed-artist?updated=1"
        );
        let body = server.get("/admin/artist/renamed-artist").await.text();
        assert!(
            body.contains("btn btn-error") && !body.contains("btn btn-secondary"),
            "the restored artist should show Delete, not Restore:\n{body}"
        );
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn artist_form_failures_carry_fixed_codes(pool: PgPool) {
        create_test_record_label(&pool, 1).await.expect("label");
        create_test_user_with_permissions(&pool, 1, vec!["admin", "label_owner"])
            .await
            .expect("admin + label_owner user");
        create_test_user_with_permissions(&pool, 2, vec!["label_owner"])
            .await
            .expect("label_owner-only user");

        let server = api_server(pool).await;
        login_as(&server, 1).await;

        // An empty name steers back to the create page with the fixed code
        // (the submitted value is never echoed), and the alert renders.
        let response = post_form(
            &server,
            "/admin/forms/artist",
            &[("label_id", "1"), ("name", "")],
        )
        .await;
        assert_eq!(location(&response), "/admin/artist?bad=name_required");
        let body = server.get("/admin/artist?bad=name_required").await.text();
        assert!(
            body.contains("Name is required."),
            "the create page should show the fixed alert:\n{body}"
        );

        // Two artists with the same name collide on the derived slug.
        let first = post_form(
            &server,
            "/admin/forms/artist",
            &[("label_id", "1"), ("name", "Dupe Artist")],
        )
        .await;
        assert_eq!(location(&first), "/admin/artist/dupe-artist");
        let dupe = post_form(
            &server,
            "/admin/forms/artist",
            &[("label_id", "1"), ("name", "Dupe Artist")],
        )
        .await;
        assert_eq!(location(&dupe), "/admin/artist?bad=slug_taken");

        // An unknown slug on delete gets the generic code; only the Location
        // is asserted (the editor at that slug renders NotFound inline).
        let response = post_form(
            &server,
            "/admin/forms/artist/delete",
            &[("slug", "no-such-artist")],
        )
        .await;
        assert_eq!(
            location(&response),
            "/admin/artist/no-such-artist?bad=server"
        );

        // The artist services require admin *and* label_owner: an owner
        // without admin gets the create page
        // served, then the service forbids the submit.
        login_as(&server, 2).await;
        let response = post_form(
            &server,
            "/admin/forms/artist",
            &[("label_id", "1"), ("name", "Owner Without Admin")],
        )
        .await;
        assert_eq!(location(&response), "/admin/artist?bad=forbidden");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn missing_artist_editor_lands_on_not_found(pool: PgPool) {
        // The profile editor's fetch failure must not land back on the
        // create page: an inline SSR redirect re-parses its Display string,
        // so the NotFound target must be a URL no route matches (a bare
        // "/admin/artist" re-parses as CreateArtist; any other slug
        // re-opens this editor). See `EditArtistBody`.
        create_test_record_label(&pool, 1).await.expect("label");
        create_test_user_with_permissions(&pool, 1, vec!["admin", "label_owner"])
            .await
            .expect("admin + label_owner user");

        let server = api_server(pool).await;
        login_as(&server, 1).await;

        let response = server.get("/admin/artist/no-such-slug").await;
        assert_shell(&response, &["Oh No...", CHROME[0], CHROME[1]]);
        let body = response.text();
        // Neither the create form nor the editor may render in its place.
        assert!(
            !body.contains("action=\"/admin/forms/artist\""),
            "must not fall back to the create page:\n{body}"
        );
        assert!(!body.contains("tabs tabs-border"), "no tab layout:\n{body}");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn release_editor_flows_for_a_label_owner(pool: PgPool) {
        let label = create_test_record_label(&pool, 1).await.expect("label");
        create_test_user_with_permissions(&pool, 1, vec!["admin", "label_owner"])
            .await
            .expect("admin + label_owner user");
        let artist = create_test_artist(&pool, 1, Some(label))
            .await
            .expect("artist fixture");
        create_test_release(&pool, 1, Some(artist))
            .await
            .expect("release fixture");

        let server = api_server(pool).await;
        login_as(&server, 1).await;

        // The list shows the seeded release (catalogue number) and its
        // editor link, plus the Add link.
        let list = server.get("/admin/artist/test-artist-1/releases").await;
        assert_shell(
            &list,
            &[
                "Test Artist 1 Releases",
                "TEST-1",
                "href=\"/admin/artist/test-artist-1/release/test-release-1?\"",
                "href=\"/admin/artist/test-artist-1/releases/new?\"",
            ],
        );

        // The create page serves the real form handlers, addressed by the
        // artist slug in the URL.
        let create_page = server.get("/admin/artist/test-artist-1/releases/new").await;
        assert_shell(
            &create_page,
            &[
                "New Release",
                "action=\"/admin/forms/release\"",
                "Main Artist",
                "All Artists",
            ],
        );

        // Create 303s silently to the editor under the slug the server
        // derived from the name.
        let created = post_form_pairs(
            &server,
            "/admin/forms/release",
            &[
                ("label_id", "1"),
                ("artist_slug", "test-artist-1"),
                ("name", "First Album"),
                ("description", "Liner notes."),
                ("artist_ids", "1"),
                ("primary_artist_id", "1"),
                ("catalogue_number", "FA-1"),
                ("local_release_date", "2026-03-01T09:30"),
                ("local_published_at", "2026-01-01T00:00"),
            ],
        )
        .await;
        assert_eq!(
            location(&created),
            "/admin/artist/test-artist-1/release/first-album"
        );

        // The editor loads the new row (catalogue value, update action, and
        // the Delete button since the row is live).
        let editor = server
            .get("/admin/artist/test-artist-1/release/first-album")
            .await;
        assert_shell(
            &editor,
            &[
                "First Album - Test Artist 1",
                "action=\"/admin/forms/release/update\"",
                "value=\"FA-1\"",
                "btn btn-error",
            ],
        );

        // Rename re-slugifies: the 303 lands on the *new* release slug, and
        // the browser then sees the success alert at that URL.
        let renamed = post_form_pairs(
            &server,
            "/admin/forms/release/update",
            &[
                ("slug", "first-album"),
                ("label_id", "1"),
                ("artist_slug", "test-artist-1"),
                ("name", "Renamed Album"),
                ("description", "Liner notes."),
                ("artist_ids", "1"),
                ("primary_artist_id", "1"),
                ("catalogue_number", "FA-1"),
                ("local_published_at", "2026-01-01T00:00"),
            ],
        )
        .await;
        assert_eq!(
            location(&renamed),
            "/admin/artist/test-artist-1/release/renamed-album?updated=1"
        );
        let body = server
            .get("/admin/artist/test-artist-1/release/renamed-album?updated=1")
            .await
            .text();
        assert!(
            body.contains("Renamed Album Updated!"),
            "the editor should show the success alert:\n{body}"
        );

        // Soft delete swaps Delete for Restore on reload (`label_owner`
        // sessions see soft-deleted rows through the service's visibility
        // check); restore swaps it back.
        let deleted = post_form(
            &server,
            "/admin/forms/release/delete",
            &[("slug", "renamed-album"), ("artist_slug", "test-artist-1")],
        )
        .await;
        assert_eq!(
            location(&deleted),
            "/admin/artist/test-artist-1/release/renamed-album?updated=1"
        );
        let body = server
            .get("/admin/artist/test-artist-1/release/renamed-album?updated=1")
            .await
            .text();
        assert!(
            body.contains("btn btn-secondary"),
            "the deleted row should offer Restore:\n{body}"
        );
        assert!(
            !body.contains("btn btn-error"),
            "no Delete button while soft-deleted:\n{body}"
        );

        let restored = post_form(
            &server,
            "/admin/forms/release/restore",
            &[("slug", "renamed-album"), ("artist_slug", "test-artist-1")],
        )
        .await;
        assert_eq!(
            location(&restored),
            "/admin/artist/test-artist-1/release/renamed-album?updated=1"
        );
        let body = server
            .get("/admin/artist/test-artist-1/release/renamed-album?updated=1")
            .await
            .text();
        assert!(
            body.contains("btn btn-error"),
            "the restored row offers Delete again:\n{body}"
        );
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn release_form_failures_carry_fixed_codes(pool: PgPool) {
        let label = create_test_record_label(&pool, 1).await.expect("label");
        create_test_user_with_permissions(&pool, 1, vec!["admin", "label_owner"])
            .await
            .expect("admin + label_owner user");
        create_test_user_with_permissions(&pool, 2, vec!["label_owner"])
            .await
            .expect("label_owner-only user");
        let artist = create_test_artist(&pool, 1, Some(label))
            .await
            .expect("artist fixture");
        // The fixture's catalogue number (TEST-1) is unique per label, so a
        // create reusing it must fail.
        create_test_release(&pool, 1, Some(artist))
            .await
            .expect("release fixture");

        let server = api_server(pool).await;
        login_as(&server, 1).await;

        // An empty name steers back to the create page with the fixed code.
        let response = post_form_pairs(
            &server,
            "/admin/forms/release",
            &[
                ("label_id", "1"),
                ("artist_slug", "test-artist-1"),
                ("name", ""),
                ("artist_ids", "1"),
                ("primary_artist_id", "1"),
            ],
        )
        .await;
        assert_eq!(
            location(&response),
            "/admin/artist/test-artist-1/releases/new?bad=name_required"
        );
        let body = server
            .get("/admin/artist/test-artist-1/releases/new?bad=name_required")
            .await
            .text();
        assert!(
            body.contains("Name is required."),
            "the create page should show the fixed alert:\n{body}"
        );

        // The catalogue number must be unique within the label.
        let dupe = post_form_pairs(
            &server,
            "/admin/forms/release",
            &[
                ("label_id", "1"),
                ("artist_slug", "test-artist-1"),
                ("name", "Dupe Catalogue"),
                ("artist_ids", "1"),
                ("primary_artist_id", "1"),
                ("catalogue_number", "TEST-1"),
            ],
        )
        .await;
        assert_eq!(
            location(&dupe),
            "/admin/artist/test-artist-1/releases/new?bad=catalogue_taken"
        );
        let body = server
            .get("/admin/artist/test-artist-1/releases/new?bad=catalogue_taken")
            .await
            .text();
        assert!(
            body.contains("Catalogue number must be unique."),
            "the create page should show the fixed alert:\n{body}"
        );

        // A primary artist the label does not have gets its own code.
        let ghost = post_form_pairs(
            &server,
            "/admin/forms/release",
            &[
                ("label_id", "1"),
                ("artist_slug", "test-artist-1"),
                ("name", "Ghost Primary"),
                ("artist_ids", "999"),
                ("primary_artist_id", "999"),
                ("catalogue_number", "GP-1"),
            ],
        )
        .await;
        assert_eq!(
            location(&ghost),
            "/admin/artist/test-artist-1/releases/new?bad=no_artist"
        );

        // An unknown slug on delete gets the generic code; only the
        // Location is asserted (the editor at that slug renders NotFound).
        let response = post_form(
            &server,
            "/admin/forms/release/delete",
            &[
                ("slug", "no-such-release"),
                ("artist_slug", "test-artist-1"),
            ],
        )
        .await;
        assert_eq!(
            location(&response),
            "/admin/artist/test-artist-1/release/no-such-release?bad=server"
        );

        // The release services require admin *and* label_owner: an owner
        // without admin gets the form served, then the service forbids the
        // submit.
        login_as(&server, 2).await;
        let response = post_form_pairs(
            &server,
            "/admin/forms/release",
            &[
                ("label_id", "1"),
                ("artist_slug", "test-artist-1"),
                ("name", "Owner Without Admin"),
                ("artist_ids", "1"),
                ("primary_artist_id", "1"),
            ],
        )
        .await;
        assert_eq!(
            location(&response),
            "/admin/artist/test-artist-1/releases/new?bad=forbidden"
        );
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn track_editor_flows_for_a_label_owner(pool: PgPool) {
        let label = create_test_record_label(&pool, 1).await.expect("label");
        create_test_user_with_permissions(&pool, 1, vec!["admin", "label_owner"])
            .await
            .expect("admin + label_owner user");
        let artist = create_test_artist(&pool, 1, Some(label))
            .await
            .expect("artist fixture");
        let release = create_test_release(&pool, 1, Some(artist))
            .await
            .expect("release fixture");
        let release_id = release.id.to_string();

        let server = api_server(pool).await;
        login_as(&server, 1).await;

        // The release editor's tracks sidebar (slice 5c) renders from the
        // editor's own release fetch; a release without tracks gets the
        // empty-state row plus the Add link.
        let editor = server
            .get("/admin/artist/test-artist-1/release/test-release-1")
            .await;
        assert_shell(
            &editor,
            &[
                "Test Release 1 - Test Artist 1",
                "No tracks found.",
                // Link renders a trailing `?` for routes with optional
                // query segments (EditTrack-style routes).
                "href=\"/admin/artist/test-artist-1/release/test-release-1/tracks/new?\"",
            ],
        );

        // The dedicated tracks page renders the same empty state (it reuses
        // the release fetch rather than a separate tracks query).
        let tracks = server
            .get("/admin/artist/test-artist-1/release/test-release-1/tracks")
            .await;
        assert_shell(&tracks, &["No tracks found."]);

        // The create page seeds the addressed release's id as a hidden
        // field and posts to the track create handler.
        let create_page = server
            .get("/admin/artist/test-artist-1/release/test-release-1/tracks/new")
            .await;
        assert_shell(
            &create_page,
            &[
                "New Track",
                "action=\"/admin/forms/track\"",
                "name=\"release_id\"",
            ],
        );

        // Create 303s silently to the new track's editor under the slug the
        // server derived from the name.
        let created = post_form_pairs(
            &server,
            "/admin/forms/track",
            &[
                ("release_id", release_id.as_str()),
                ("artist_slug", "test-artist-1"),
                ("release_slug", "test-release-1"),
                ("name", "Test Song"),
                ("description", "Verse one."),
                ("lyrics", "# Verse one"),
                ("artist_ids", "1"),
                ("primary_artist_id", "1"),
                ("isrc_code", "USRC17607839"),
                ("bpm", "120"),
                ("track_number", "1"),
                ("local_published_at", "2026-01-01T00:00"),
            ],
        )
        .await;
        assert_eq!(
            location(&created),
            "/admin/artist/test-artist-1/release/test-release-1/track/test-song"
        );

        // The editor loads the new row (track-release-artist header, update
        // action, ISRC and bpm values, and Delete since the row is live).
        let editor = server
            .get("/admin/artist/test-artist-1/release/test-release-1/track/test-song")
            .await;
        assert_shell(
            &editor,
            &[
                "Test Song - Test Release 1 - Test Artist 1",
                "action=\"/admin/forms/track/update\"",
                "value=\"USRC17607839\"",
                "value=\"120\"",
                "btn btn-error",
            ],
        );

        // The release editor's sidebar now carries the new row link.
        let release_editor = server
            .get("/admin/artist/test-artist-1/release/test-release-1")
            .await;
        assert_shell(
            &release_editor,
            &["href=\"/admin/artist/test-artist-1/release/test-release-1/track/test-song?\""],
        );

        // Rename re-slugifies: the 303 lands on the *new* track slug with
        // `updated=1`, and the browser then sees the success alert there.
        let renamed = post_form_pairs(
            &server,
            "/admin/forms/track/update",
            &[
                ("release_id", release_id.as_str()),
                ("slug", "test-song"),
                ("artist_slug", "test-artist-1"),
                ("release_slug", "test-release-1"),
                ("name", "Renamed Song"),
                ("description", "Verse one."),
                ("lyrics", "# Verse one"),
                ("artist_ids", "1"),
                ("primary_artist_id", "1"),
                ("isrc_code", "USRC17607839"),
                ("bpm", "120"),
                ("track_number", "1"),
                ("local_published_at", "2026-01-01T00:00"),
            ],
        )
        .await;
        assert_eq!(
            location(&renamed),
            "/admin/artist/test-artist-1/release/test-release-1/track/renamed-song?updated=1"
        );
        let body = server
            .get("/admin/artist/test-artist-1/release/test-release-1/track/renamed-song?updated=1")
            .await
            .text();
        assert!(
            body.contains("Renamed Song Updated!"),
            "the editor should show the success alert:\n{body}"
        );

        // Soft delete swaps Delete for Restore on reload (`label_owner`
        // sessions see soft-deleted rows through the service's visibility
        // check, so the editor keeps working for Restore); restore swaps it
        // back.
        let deleted = post_form(
            &server,
            "/admin/forms/track/delete",
            &[
                ("slug", "renamed-song"),
                ("artist_slug", "test-artist-1"),
                ("release_slug", "test-release-1"),
            ],
        )
        .await;
        assert_eq!(
            location(&deleted),
            "/admin/artist/test-artist-1/release/test-release-1/track/renamed-song?updated=1"
        );
        let body = server
            .get("/admin/artist/test-artist-1/release/test-release-1/track/renamed-song?updated=1")
            .await
            .text();
        assert!(
            body.contains("btn btn-secondary"),
            "the deleted track should offer Restore:\n{body}"
        );
        assert!(
            !body.contains("btn btn-error"),
            "no Delete button while soft-deleted:\n{body}"
        );

        let restored = post_form(
            &server,
            "/admin/forms/track/restore",
            &[
                ("slug", "renamed-song"),
                ("artist_slug", "test-artist-1"),
                ("release_slug", "test-release-1"),
            ],
        )
        .await;
        assert_eq!(
            location(&restored),
            "/admin/artist/test-artist-1/release/test-release-1/track/renamed-song?updated=1"
        );
        let body = server
            .get("/admin/artist/test-artist-1/release/test-release-1/track/renamed-song?updated=1")
            .await
            .text();
        assert!(
            body.contains("btn btn-error"),
            "the restored row offers Delete again:\n{body}"
        );
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn track_form_failures_carry_fixed_codes(pool: PgPool) {
        let label = create_test_record_label(&pool, 1).await.expect("label");
        create_test_user_with_permissions(&pool, 1, vec!["admin", "label_owner"])
            .await
            .expect("admin + label_owner user");
        create_test_user_with_permissions(&pool, 2, vec!["label_owner"])
            .await
            .expect("label_owner-only user");
        let artist = create_test_artist(&pool, 1, Some(label))
            .await
            .expect("artist fixture");
        let release = create_test_release(&pool, 1, Some(artist))
            .await
            .expect("release fixture");
        let release_id = release.id.to_string();

        let server = api_server(pool).await;
        login_as(&server, 1).await;

        // An empty name steers back to the create page with the fixed code.
        let response = post_form_pairs(
            &server,
            "/admin/forms/track",
            &[
                ("release_id", release_id.as_str()),
                ("artist_slug", "test-artist-1"),
                ("release_slug", "test-release-1"),
                ("name", ""),
                ("artist_ids", "1"),
                ("primary_artist_id", "1"),
            ],
        )
        .await;
        assert_eq!(
            location(&response),
            "/admin/artist/test-artist-1/release/test-release-1/tracks/new?bad=name_required"
        );
        let body = server
            .get("/admin/artist/test-artist-1/release/test-release-1/tracks/new?bad=name_required")
            .await
            .text();
        assert!(
            body.contains("Name is required."),
            "the create page should show the fixed alert:\n{body}"
        );

        // An ISRC that is present but not 12 characters gets its own code.
        let response = post_form_pairs(
            &server,
            "/admin/forms/track",
            &[
                ("release_id", release_id.as_str()),
                ("artist_slug", "test-artist-1"),
                ("release_slug", "test-release-1"),
                ("name", "Short ISRC"),
                ("artist_ids", "1"),
                ("primary_artist_id", "1"),
                ("isrc_code", "ABC"),
            ],
        )
        .await;
        assert_eq!(
            location(&response),
            "/admin/artist/test-artist-1/release/test-release-1/tracks/new?bad=isrc_length"
        );

        // A blank ISRC (the form always submits the field) fails the same
        // length check instead of hitting the NOT NULL column.
        let response = post_form_pairs(
            &server,
            "/admin/forms/track",
            &[
                ("release_id", release_id.as_str()),
                ("artist_slug", "test-artist-1"),
                ("release_slug", "test-release-1"),
                ("name", "Blank ISRC"),
                ("artist_ids", "1"),
                ("primary_artist_id", "1"),
                ("isrc_code", ""),
            ],
        )
        .await;
        assert_eq!(
            location(&response),
            "/admin/artist/test-artist-1/release/test-release-1/tracks/new?bad=isrc_length"
        );

        // The first use of an ISRC is accepted (silent create redirect).
        let first = post_form_pairs(
            &server,
            "/admin/forms/track",
            &[
                ("release_id", release_id.as_str()),
                ("artist_slug", "test-artist-1"),
                ("release_slug", "test-release-1"),
                ("name", "First ISRC Song"),
                ("artist_ids", "1"),
                ("primary_artist_id", "1"),
                ("isrc_code", "USRC17607839"),
                ("track_number", "8"),
            ],
        )
        .await;
        assert_eq!(
            location(&first),
            "/admin/artist/test-artist-1/release/test-release-1/track/first-isrc-song"
        );

        // Reusing the ISRC on another track fails with the uniqueness code.
        let dupe = post_form_pairs(
            &server,
            "/admin/forms/track",
            &[
                ("release_id", release_id.as_str()),
                ("artist_slug", "test-artist-1"),
                ("release_slug", "test-release-1"),
                ("name", "Second ISRC Song"),
                ("artist_ids", "1"),
                ("primary_artist_id", "1"),
                ("isrc_code", "USRC17607839"),
                ("track_number", "9"),
            ],
        )
        .await;
        assert_eq!(
            location(&dupe),
            "/admin/artist/test-artist-1/release/test-release-1/tracks/new?bad=isrc_taken"
        );

        // Track numbers must be unique within the release.
        let number_dupe = post_form_pairs(
            &server,
            "/admin/forms/track",
            &[
                ("release_id", release_id.as_str()),
                ("artist_slug", "test-artist-1"),
                ("release_slug", "test-release-1"),
                ("name", "Number Collision"),
                ("artist_ids", "1"),
                ("primary_artist_id", "1"),
                ("isrc_code", "QYZZ18111111"),
                ("track_number", "8"),
            ],
        )
        .await;
        assert_eq!(
            location(&number_dupe),
            "/admin/artist/test-artist-1/release/test-release-1/tracks/new?bad=track_number_taken"
        );

        // An unknown slug on delete gets the generic code; only the
        // Location is asserted (the editor at that slug redirects back to
        // the tracks list rather than rendering a form).
        let response = post_form(
            &server,
            "/admin/forms/track/delete",
            &[
                ("slug", "no-such-track"),
                ("artist_slug", "test-artist-1"),
                ("release_slug", "test-release-1"),
            ],
        )
        .await;
        assert_eq!(
            location(&response),
            "/admin/artist/test-artist-1/release/test-release-1/track/no-such-track?bad=server"
        );

        // The track services require admin *and* label_owner: an owner
        // without admin gets the form served, then the service forbids the
        // submit.
        login_as(&server, 2).await;
        let response = post_form_pairs(
            &server,
            "/admin/forms/track",
            &[
                ("release_id", release_id.as_str()),
                ("artist_slug", "test-artist-1"),
                ("release_slug", "test-release-1"),
                ("name", "Owner Without Admin"),
                ("artist_ids", "1"),
                ("primary_artist_id", "1"),
            ],
        )
        .await;
        assert_eq!(
            location(&response),
            "/admin/artist/test-artist-1/release/test-release-1/tracks/new?bad=forbidden"
        );
    }

    /// Slice 5d: the links editor page and its no-JS form handler. The GET
    /// renders one input per platform, the 303 handler
    /// saves and steers back with `?updated=1`, and failures carry fixed
    /// codes.
    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn links_editor_flow_for_a_label_owner(pool: PgPool) {
        let label = create_test_record_label(&pool, 1).await.expect("label");
        create_test_user_with_permissions(&pool, 1, vec!["admin", "label_owner"])
            .await
            .expect("admin + label_owner user");
        create_test_user_with_permissions(&pool, 2, vec!["label_owner"])
            .await
            .expect("label_owner-only user");
        create_test_artist(&pool, 1, Some(label))
            .await
            .expect("artist fixture");

        let server = api_server(pool).await;
        login_as(&server, 1).await;

        let page = server.get("/admin/artist/test-artist-1/links").await;
        assert_shell(
            &page,
            &[
                "Test Artist 1 Links",
                "action=\"/admin/forms/links\"",
                "name=\"spotify\"",
                "src=\"/images/music_services/Spotify.svg\"",
            ],
        );

        // A save 303s back with `?updated=1`; GETing that URL renders both
        // the alert and the saved value in the input.
        let saved = post_form_pairs(
            &server,
            "/admin/forms/links",
            &[
                ("artist_slug", "test-artist-1"),
                ("spotify", "https://open.spotify.com/artist/test"),
            ],
        )
        .await;
        assert_eq!(
            location(&saved),
            "/admin/artist/test-artist-1/links?updated=1"
        );
        let body = server
            .get("/admin/artist/test-artist-1/links?updated=1")
            .await
            .text();
        assert!(
            body.contains("Test Artist 1 Links Updated!"),
            "the success alert should render:\n{body}"
        );
        assert!(
            body.contains("value=\"https://open.spotify.com/artist/test\""),
            "the saved url should pre-fill the input:\n{body}"
        );

        // Clearing the input deletes the row (empty field + saved row).
        let cleared = post_form_pairs(
            &server,
            "/admin/forms/links",
            &[("artist_slug", "test-artist-1"), ("spotify", "")],
        )
        .await;
        assert_eq!(cleared.status_code(), StatusCode::SEE_OTHER);
        let body = server
            .get("/admin/artist/test-artist-1/links?updated=1")
            .await
            .text();
        assert!(
            !body.contains("https://open.spotify.com/artist/test"),
            "the cleared row must not come back:\n{body}"
        );

        // An unknown artist gets `no_artist`; the page gate only requires
        // label_owner, but the service also needs admin, so a label_owner
        // without admin gets `forbidden`.
        let ghost = post_form_pairs(
            &server,
            "/admin/forms/links",
            &[("artist_slug", "ghost"), ("spotify", "https://example.com")],
        )
        .await;
        assert_eq!(location(&ghost), "/admin/artist/ghost/links?bad=no_artist");
        login_as(&server, 2).await;
        let forbidden = post_form_pairs(
            &server,
            "/admin/forms/links",
            &[
                ("artist_slug", "test-artist-1"),
                ("spotify", "https://example.com"),
            ],
        )
        .await;
        assert_eq!(
            location(&forbidden),
            "/admin/artist/test-artist-1/links?bad=forbidden"
        );
    }

    /// The JSON twin `POST /api/update_links` (the reactive path the wasm
    /// client will call in Phases 6-7) saves, clears and authorizes like
    /// the form handler, answering with the `LinksResult` the editor
    /// re-renders.
    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn update_links_twin_saves_and_clears_links(pool: PgPool) {
        let label = create_test_record_label(&pool, 1).await.expect("label");
        create_test_user_with_permissions(&pool, 1, vec!["admin", "label_owner"])
            .await
            .expect("admin + label_owner user");
        create_test_user_with_permissions(&pool, 2, vec!["label_owner"])
            .await
            .expect("label_owner-only user");
        create_test_artist(&pool, 1, Some(label))
            .await
            .expect("artist fixture");

        let server = api_server(pool.clone()).await;
        login_as(&server, 1).await;

        // The dioxus client sends the server-fn args as a JSON object of
        // named parameters: `{"form": …}`.
        let saved = server
            .post("/api/update_links")
            .json(&serde_json::json!({
                "form": {
                    "artist_slug": "test-artist-1",
                    "spotify": "https://example.com/artist",
                }
            }))
            .await;
        saved.assert_status_ok();
        let body = saved.text();
        assert!(
            body.contains("https://example.com/artist"),
            "the LinksResult should carry the saved url:\n{body}"
        );

        // The GET twin's typed args ride in the query string, which the
        // route literal declares (`?artist_slug`): browsers refuse a body
        // on GET, so the dioxus client stub can only send declared params
        // in the URL.
        let fetched = server
            .get("/api/get_links?artist_slug=test-artist-1")
            .await
            .text();
        assert!(
            fetched.contains("https://example.com/artist"),
            "get_links should return the saved url:\n{fetched}"
        );

        // An empty field deletes the saved row.
        let cleared = server
            .post("/api/update_links")
            .json(&serde_json::json!({
                "form": {
                    "artist_slug": "test-artist-1",
                    "spotify": "",
                }
            }))
            .await;
        cleared.assert_status_ok();
        let body = cleared.text();
        assert!(
            !body.contains("https://example.com/artist"),
            "the cleared row must be gone from the LinksResult:\n{body}"
        );

        // The service's NotFound for an unknown artist surfaces as 404.
        let ghost = server
            .post("/api/update_links")
            .json(&serde_json::json!({
                "form": { "artist_slug": "ghost", "spotify": "https://example.com" }
            }))
            .await;
        assert_eq!(ghost.status_code(), StatusCode::NOT_FOUND);

        // A guest hits the user extractor first (401), a label_owner
        // without admin reaches the permission check (403).
        let guest_server = api_server(pool).await;
        let unauthorized = guest_server
            .post("/api/update_links")
            .json(&serde_json::json!({
                "form": { "artist_slug": "test-artist-1", "spotify": "https://example.com" }
            }))
            .await;
        assert_eq!(unauthorized.status_code(), StatusCode::UNAUTHORIZED);

        login_as(&server, 2).await;
        let forbidden = server
            .post("/api/update_links")
            .json(&serde_json::json!({
                "form": { "artist_slug": "test-artist-1", "spotify": "https://example.com" }
            }))
            .await;
        assert_eq!(forbidden.status_code(), StatusCode::FORBIDDEN);
    }

    // --- Reactive editor twins (`/api/*_{create,update,delete,restore}_…`) --
    //
    // A desktop window cannot follow the `?updated=1` / `?bad=<code>` redirects
    // the admin handlers answer with, so these twins carry the same outcome as
    // data: `Saved` with the page's own result type (which the editor then
    // renders in place of a refetch), or `Failed` with the very same code.

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn release_twins_answer_the_saved_row_and_the_form_codes(pool: PgPool) {
        let label = create_test_record_label(&pool, 1).await.expect("label");
        let label_id = label.id;
        create_test_user_with_permissions(&pool, 1, vec!["admin", "label_owner"])
            .await
            .expect("admin + label_owner user");
        let artist = create_test_artist(&pool, 1, Some(label))
            .await
            .expect("artist fixture");
        let artist_id = artist.id;
        create_test_release(&pool, 1, Some(artist))
            .await
            .expect("release fixture");

        let server = api_server(pool.clone()).await;
        login_as(&server, 1).await;

        // The dioxus client sends the server-fn arguments by name, so the body
        // key here is the twin's argument name (`fields` for the update form).
        let answer = server
            .post("/api/update_release")
            .json(&serde_json::json!({
                "fields": {
                    "slug": "test-release-1",
                    "artist_ids": artist_id.to_string(),
                    "label_id": label_id.to_string(),
                    "artist_slug": "test-artist-1",
                    "name": "Renamed Album",
                    "description": "Liner notes.",
                    "catalogue_number": "TEST-1",
                    "primary_artist_id": artist_id.to_string(),
                }
            }))
            .await;
        answer.assert_status_ok();
        let saved: serde_json::Value = answer.json();
        // A rename re-sluggifies, and the answer carries the row as the server
        // now holds it — the slug the editor's next save and uploader target.
        assert_eq!(
            saved["Saved"]["release"]["slug"].as_str(),
            Some("renamed-album"),
            "the saved row should carry the new slug:\n{saved}"
        );
        assert!(
            saved["Saved"]["artists"].is_array(),
            "the page's whole result comes back, artists included:\n{saved}"
        );

        // A refusal carries the code the 303 handler puts in `?bad=`, which the
        // editor maps through the same alert table it uses for the query.
        let answer = server
            .post("/api/update_release")
            .json(&serde_json::json!({
                "fields": {
                    "slug": "renamed-album",
                    "artist_ids": artist_id.to_string(),
                    "label_id": label_id.to_string(),
                    "artist_slug": "test-artist-1",
                    "name": "",
                    "primary_artist_id": artist_id.to_string(),
                }
            }))
            .await;
        answer.assert_status_ok();
        let refused: serde_json::Value = answer.json();
        assert_eq!(
            refused["Failed"]["code"].as_str(),
            Some("name_required"),
            "the empty name should be refused with the form's code:\n{refused}"
        );

        // Delete and restore answer the same way: the row in the answer carries
        // `deleted_at`, which is what swaps the editor's Delete button for
        // Restore without a page load.
        let answer = server
            .post("/api/delete_release")
            .json(&serde_json::json!({
                "target": { "slug": "renamed-album", "artist_slug": "test-artist-1" }
            }))
            .await;
        answer.assert_status_ok();
        let deleted: serde_json::Value = answer.json();
        assert!(
            deleted["Saved"]["release"]["deleted_at"].is_string(),
            "the deleted row should carry deleted_at:\n{deleted}"
        );

        let answer = server
            .post("/api/restore_release")
            .json(&serde_json::json!({
                "target": { "slug": "renamed-album", "artist_slug": "test-artist-1" }
            }))
            .await;
        answer.assert_status_ok();
        let restored: serde_json::Value = answer.json();
        assert_eq!(
            restored["Saved"]["release"]["deleted_at"].as_str(),
            None,
            "the restored row should be live again:\n{restored}"
        );

        // A hand-crafted post with no session gets the steer the 303 handler
        // would have given, as an answer a client can act on.
        let guest_server = api_server(pool).await;
        let unauthorized = guest_server
            .post("/api/delete_release")
            .json(&serde_json::json!({
                "target": { "slug": "test-release-1", "artist_slug": "test-artist-1" }
            }))
            .await;
        assert_eq!(unauthorized.status_code(), StatusCode::UNAUTHORIZED);
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn label_twin_answers_the_saved_label_and_the_forbidden_code(pool: PgPool) {
        let label = create_test_record_label(&pool, 1).await.expect("label");
        create_test_user_with_permissions(&pool, 1, vec!["admin", "label_owner"])
            .await
            .expect("admin + label_owner user");
        // `admin` alone is not enough; the service checks `label_owner`.
        create_test_user_with_permissions(&pool, 2, vec!["admin"])
            .await
            .expect("admin-only user");

        let server = api_server(pool.clone()).await;
        login_as(&server, 1).await;

        let answer = server
            .post("/api/update_record_label")
            .json(&serde_json::json!({
                "fields": {
                    "id": label.id.to_string(),
                    "name": "Renamed Label",
                    "description": "new story",
                    "isrc_base": "US ABC 1",
                }
            }))
            .await;
        answer.assert_status_ok();
        let saved: serde_json::Value = answer.json();
        assert_eq!(
            saved["Saved"]["record_label"]["name"].as_str(),
            Some("Renamed Label"),
            "the header follows the saved name:\n{saved}"
        );

        login_as(&server, 2).await;
        let answer = server
            .post("/api/update_record_label")
            .json(&serde_json::json!({
                "fields": { "id": label.id.to_string(), "name": "Hijacked Label" }
            }))
            .await;
        answer.assert_status_ok();
        let refused: serde_json::Value = answer.json();
        assert_eq!(
            refused["Failed"]["code"].as_str(),
            Some("forbidden"),
            "the service's refusal should come back as data:\n{refused}"
        );
    }

    /// The artist Images tab (Phase 5 slice 5g): the SSR body carries the
    /// upload widget addressed to the artist slug, and the artist's current
    /// primary image below it (the site logo until one is stored).
    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn artist_images_page_renders_the_uploader(pool: PgPool) {
        let label = create_test_record_label(&pool, 1)
            .await
            .expect("label fixture");
        let artist = create_test_artist(&pool, 1, Some(label))
            .await
            .expect("artist fixture");
        create_test_user_with_permissions(&pool, 1, vec!["admin", "label_owner"])
            .await
            .expect("admin + label_owner user");

        // `api_server` takes the pool; `PgPool` is an Arc handle, so the
        // clone leaves this test's pool usable for the update below.
        let server = api_server(pool.clone()).await;
        login_as(&server, 1).await;

        let page = server.get("/admin/artist/test-artist-1/images").await;
        assert_shell(
            &page,
            &[
                "Test Artist 1 Images",
                "name=\"file_to_upload\"",
                "class=\"w-full file-input file-input-primary\"",
                ">Upload</button>",
                // The Artist configuration drives the accept list, as extension
                // tokens (a window's file dialog cannot read mime types).
                "accept=\".jpg,.JPG,.jpeg,.JPEG,.png,.PNG,.gif,.GIF,.webp,.WEBP\"",
                // Nothing stored yet: the model's default logo.
                "src=\"/Logo.svg\"",
                "alt=\"Test Artist 1\"",
            ],
        );

        // With a stored file the same page points at it — the URL the widget's
        // `on_uploaded` handler builds after an upload.
        sqlx::query("UPDATE artists SET primary_image = $1 WHERE id = $2")
            .bind("1700000000-test-artist-1.png")
            .bind(artist.id)
            .execute(&pool)
            .await
            .expect("stored image");
        assert_shell(
            &server.get("/admin/artist/test-artist-1/images").await,
            &["src=\"/uploads/artists/1700000000-test-artist-1.png\""],
        );
    }

    /// The release editor's image uploader (slice 5g) renders below the field
    /// form — a sibling, never nested inside it — with the release's current
    /// image under it.
    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn release_editor_renders_the_image_uploader(pool: PgPool) {
        let label = create_test_record_label(&pool, 1)
            .await
            .expect("label fixture");
        let artist = create_test_artist(&pool, 1, Some(label))
            .await
            .expect("artist fixture");
        create_test_release(&pool, 1, Some(artist))
            .await
            .expect("release fixture");
        create_test_user_with_permissions(&pool, 1, vec!["admin", "label_owner"])
            .await
            .expect("admin + label_owner user");

        let server = api_server(pool).await;
        login_as(&server, 1).await;

        let page = server
            .get("/admin/artist/test-artist-1/release/test-release-1")
            .await;
        assert_shell(
            &page,
            &[
                "name=\"file_to_upload\"",
                // The Release configuration, addressed by the release slug.
                "accept=\".jpg,.JPG,.jpeg,.JPEG,.png,.PNG,.gif,.GIF,.webp,.WEBP\"",
                "src=\"/Logo.svg\"",
                "alt=\"Test Release 1\"",
            ],
        );

        // Ordering matters: the uploader form comes after the field form's
        // Update button, so the two forms stay siblings.
        let body = page.text();
        let update = body
            .find(">Update</button>")
            .expect("the field form's Update button");
        let images = body
            .find("class=\"divider\">Images</div>")
            .expect("the images divider");
        assert!(
            update < images,
            "the uploader should follow the field form:\n{body}"
        );
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn nested_admin_layouts_render_outside_in(pool: PgPool) {
        // Slice 5a made the profile editor real, so this check now runs with
        // an admin + label_owner session and a seeded artist (the tab layout
        // is `AdminArtistRoot`, still in `ui::placeholders`).
        create_test_user_with_permissions(&pool, 1, vec!["admin", "label_owner"])
            .await
            .expect("admin + label_owner user");
        let label = create_test_record_label(&pool, 1)
            .await
            .expect("label fixture");
        create_test_artist(&pool, 1, Some(label))
            .await
            .expect("artist fixture");
        let server = api_server(pool).await;
        login_as(&server, 1).await;

        let response = server.get("/admin/artist/test-artist-1").await;

        // AppShell chrome, AdminRoot menu + article wrapper,
        // AdminArtistRoot tabs, and the real profile editor — with the slug
        // prop reaching the child component through the seeded values.
        assert_shell(
            &response,
            &[
                CHROME[0],
                CHROME[1],
                "w-56 menu bg-base-200 rounded-box",
                "tabs tabs-border",
                "href=\"/admin/artist/test-artist-1/releases\"",
                "tab tab-active",
                "Test Artist 1 Profile",
                "value=\"Test Artist 1\"",
            ],
        );

        // Slice 5b: the Releases tab now serves the real list page (the
        // seeded artist has no releases yet, hence the empty-state row).
        let releases = server.get("/admin/artist/test-artist-1/releases").await;
        assert_shell(
            &releases,
            &[
                "tabs tabs-border",
                "Test Artist 1 Releases",
                "No releases found.",
            ],
        );

        // Slice 5d: the Links tab inside the artist tab layout.
        let links = server.get("/admin/artist/test-artist-1/links").await;
        assert_shell(
            &links,
            &[
                "tabs tabs-border",
                "Test Artist 1 Links",
                "action=\"/admin/forms/links\"",
            ],
        );
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn deep_nested_routes_and_trailing_slashes_serve(pool: PgPool) {
        create_test_user_with_permissions(&pool, 1, vec!["admin", "label_owner"])
            .await
            .expect("admin + label_owner user");
        let label = create_test_record_label(&pool, 1)
            .await
            .expect("label fixture");
        let artist = create_test_artist(&pool, 1, Some(label))
            .await
            .expect("artist fixture");
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .expect("release fixture");
        create_test_track(&pool, 1, Some(release), Some(artist))
            .await
            .expect("track fixture");
        let server = api_server(pool).await;
        login_as(&server, 1).await;

        // Slice 5c made the track editor real: it loads the seeded row all
        // the way down the artist/release/track path.
        let deep = server
            .get("/admin/artist/test-artist-1/release/test-release-1/track/test-track-1")
            .await;
        assert_shell(&deep, &["Test Track 1 - Test Release 1 - Test Artist 1"]);

        // With the editors real, a URL addressing slugs that were never
        // seeded renders the NotFound page (the editor's failed fetch
        // redirects there in place of the form).
        let ghost = server
            .get("/admin/artist/jimi/release/axis/track/castles")
            .await;
        assert_shell(&ghost, &["Oh No..."]);

        // The release editor now carries the real tracks sidebar, with a
        // row linking the seeded track.
        let release_editor = server
            .get("/admin/artist/test-artist-1/release/test-release-1")
            .await;
        assert_shell(
            &release_editor,
            &[
                "Test Release 1 - Test Artist 1",
                "href=\"/admin/artist/test-artist-1/release/test-release-1/track/test-track-1?\"",
            ],
        );

        // One route variant parses both the exact and trailing-slash forms
        // of a path (MIGRATION.md risk 4); the trailing-slash profile URL
        // now serves the real editor.
        let slashed = server.get("/admin/artist/test-artist-1/").await;
        assert_shell(
            &slashed,
            &["Test Artist 1 Profile", "value=\"Test Artist 1\""],
        );
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn unmatched_paths_render_the_not_found_page(pool: PgPool) {
        let server = test_server(pool, None).await;

        let response = server.get("/totally/bogus/path").await;

        // The catch-all still renders inside the shell; the 404 *page* is
        // served with 200 there
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
