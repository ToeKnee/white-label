//! Phase 0 migration spike: validates the Dioxus 0.7 fullstack plumbing
//! against the live Leptos white-label backend (same Postgres database and
//! the same `users` / `user_permissions` / `axum_sessions` tables).
//!
//! This is validation code for the migration (see `MIGRATION.md`), not
//! something to carry into the app itself.
//!
//! Web (SSR + hydration), serving on 127.0.0.1:8080:
//! ```sh
//! dx serve --platform web     # or: cargo run
//! ```
//! Desktop client talking to that running server:
//! ```sh
//! dx serve --platform desktop --features desktop
//! ```
//! See `spike/README.md` for the full Phase 0 checklist and findings.

use dioxus::prelude::*;

mod api;
mod models;
mod routes;

#[cfg(feature = "server")]
mod auth;
#[cfg(feature = "server")]
mod db;

fn main() {
    // Server build: create the axum router by hand (database, session store,
    // auth layer) and mount the Dioxus app on it, as in Dioxus' own
    // `07-fullstack/auth` example.
    #[cfg(feature = "server")]
    dioxus::serve(|| async {
        use axum_session::{SessionConfig, SessionLayer, SessionStore};
        use axum_session_auth::AuthConfig;
        use axum_session_sqlx::SessionPgPool;

        let _ = dotenvy::dotenv();
        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
            )
            .try_init();

        let database_url = std::env::var("DATABASE_URL")
            .map_err(|_| anyhow::anyhow!("DATABASE_URL is not set (check the .env file)"))?;
        let pool = sqlx::PgPool::connect(&database_url).await?;

        // Same table and config as the Leptos app, so sessions are shared
        // while both servers run against the same database.
        let session_config = SessionConfig::default().with_table_name("axum_sessions");
        let auth_config = AuthConfig::<i64>::default();
        let session_store = SessionStore::<SessionPgPool>::new(
            Some(SessionPgPool::from(pool.clone())),
            session_config,
        )
        .await?;

        let mut router = dioxus::server::router(app)
            .layer(auth::AuthLayer::new(Some(pool.clone())).with_config(auth_config))
            .layer(SessionLayer::new(session_store))
            // Hand the pool to server functions as an extractor argument.
            .layer(axum::Extension(db::Db(pool)));

        // Serve uploaded files the same way the Leptos app does. Nested after
        // the layers so the session stack does not run for static files.
        if let Ok(upload_path) = std::env::var("UPLOAD_PATH") {
            router =
                router.nest_service("/uploads", tower_http::services::ServeDir::new(upload_path));
        }

        Ok(router)
    });

    // Client builds (web wasm, desktop, mobile): plain launch; the generated
    // stubs of the server functions call the API over HTTP.
    #[cfg(not(feature = "server"))]
    {
        #[cfg(feature = "desktop")]
        dioxus::fullstack::set_server_url("http://127.0.0.1:8080");

        dioxus::launch(app);
    }
}

fn app() -> Element {
    rsx! {
        Stylesheet { href: asset!("/assets/main.css") }
        Router::<routes::Route> {}
    }
}
