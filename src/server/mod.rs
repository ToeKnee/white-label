//! Dioxus server side of the migration (see MIGRATION.md, Phase 1).
//!
//! Assembles the axum router that `dioxus::serve` wraps: the app database
//! pool as a request extension, `tower-sessions` backed by Postgres, static
//! uploads, and the Dioxus SSR entry. The Leptos `setup.rs` still serves the
//! shipped site until cutover.

pub mod current_user;

use anyhow::Context;
use axum::Router;
use tower_sessions::SessionManagerLayer;
use tower_sessions_sqlx_store::PostgresStore;

use crate::database::create_pool;
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
/// # Errors
///
/// Returns an error if the session store can't be configured or migrated.
/// (The pool itself exits the process on failure, matching `create_pool`.)
pub async fn build_router() -> anyhow::Result<Router> {
    let pool = create_pool().await;

    let session_store = PostgresStore::new(pool.clone())
        .with_table_name(SESSION_TABLE)
        .map_err(|error| anyhow::anyhow!("invalid session table name: {error}"))?;
    session_store
        .migrate()
        .await
        .context("could not create/migrate the sessions table")?;

    let mut router = dioxus::server::router(skeleton_app)
        .layer(SessionManagerLayer::new(session_store))
        // Handlers and server fns reach the pool via `Db` (Phase 2).
        .layer(axum::Extension(Db(pool)));

    // Uploaded files, mounted after the session stack so static hits skip
    // the database — same treatment as the Leptos app's `/uploads`.
    if let Ok(upload_path) = std::env::var("UPLOAD_PATH") {
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
