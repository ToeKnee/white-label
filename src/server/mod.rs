//! Dioxus server side of the migration (see MIGRATION.md, Phase 1).
//!
//! Assembles the axum router that `dioxus::serve` wraps: the app database
//! pool as a request extension, `tower-sessions` backed by Postgres, static
//! uploads, and the Dioxus SSR entry. The Leptos `setup.rs` still serves the
//! shipped site until cutover.

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

    let mut router = dioxus::server::router(skeleton_app)
        // Ported `/api/*` endpoints (see `api` module); merged before the
        // layers below so handlers get the session and `Db` extensions.
        .merge(api::api_routes())
        .layer(SessionManagerLayer::new(session_store))
        // Handlers and server fns reach the pool via `Db`.
        .layer(axum::Extension(Db(pool)));

    // Uploaded files, mounted after the session stack so static hits skip
    // the database — same treatment as the Leptos app's `/uploads`.
    if let Some(upload_path) = upload_path {
        router = router.nest_service("/uploads", tower_http::services::ServeDir::new(upload_path));
    }

    Ok(router)
}

/// Placeholder SSR root. The real router, layouts and components arrive in
/// Phases 2–4; this only proves the Dioxus SSR + session stack boots.
fn skeleton_app() -> dioxus::prelude::Element {
    use dioxus::prelude::*;
    rsx! {
        // Dioxus `serve` wraps this in the document shell itself — no
        // `html`/`head`/`body` tags in rsx.
        main {
            h1 { "Dioxus server skeleton" }
            p {
                "Sessions, uploads and the database pool are live. The UI still runs on Leptos — see MIGRATION.md, Phases 2-4."
            }
        }
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
    use axum_test::TestServer;
    use sqlx::PgPool;

    use super::ensure_exe_public_dir;
    use crate::server::build_router;

    /// Build the real router and wrap it for in-process HTTP.
    async fn test_server(pool: PgPool, upload_path: Option<std::path::PathBuf>) -> TestServer {
        ensure_exe_public_dir();
        let router = build_router(pool, upload_path)
            .await
            .expect("router should build against the test pool");
        TestServer::new(router).expect("axum-test should wrap the router")
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn root_serves_the_dioxus_skeleton(pool: PgPool) {
        let server = test_server(pool, None).await;

        let response = server.get("/").await;

        response.assert_status_ok();
        assert!(
            response.text().contains("Dioxus server skeleton"),
            "the SSR root should render the skeleton app"
        );
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
