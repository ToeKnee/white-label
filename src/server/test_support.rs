//! In-process HTTP scaffolding shared by the server's own tests.
//!
//! Every test drives the real router from [`crate::server::build_router`], so
//! sessions, the `Db`/`UploadRoot` extensions and the Dioxus server fns behave
//! exactly as they do under `dioxus-server`.
//!
//! Example:
//!
//! ```ignore
//! let server = api_server(pool).await;
//! login_as(&server, "test-1").await;
//! let body = server.get("/artists").await.text();
//! ```

use axum_test::{TestRequest, TestServer, TestServerConfig};
use sqlx::PgPool;

use crate::server::build_router;

/// Build the real router (server fns, SSR root, forms, statics) and wrap it
/// for in-process requests. `save_cookies` makes `TestServer` replay
/// `Set-Cookie` between requests, i.e. it behaves like a browser.
pub async fn api_server(pool: PgPool) -> TestServer {
    crate::server::ensure_exe_public_dir();
    let router = build_router(pool, None)
        .await
        .expect("router should build against the test pool");
    let config = TestServerConfig {
        save_cookies: true,
        ..TestServerConfig::default()
    };
    TestServer::new_with_config(router, config)
}

/// Like [`api_server`], but with an uploads root (the upload endpoints resolve
/// theirs through `UploadRoot`).
pub async fn api_server_with_uploads(pool: &PgPool, upload_root: std::path::PathBuf) -> TestServer {
    crate::server::ensure_exe_public_dir();
    let router = build_router(pool.clone(), Some(upload_root))
        .await
        .expect("router should build against the test pool");
    let config = TestServerConfig {
        save_cookies: true,
        ..TestServerConfig::default()
    };
    TestServer::new_with_config(router, config)
}

/// The login server fn's request, un-awaited: the call site `.await`s it (and
/// usually asserts), so a test reads like the browser flow it stands for.
/// `save_cookies` on [`api_server`] then carries the session into later calls.
pub fn login_request(server: &TestServer, username: &str) -> TestRequest {
    server
        // The login server fn's path (`ui::auth::login`).
        .post("/api/login")
        .json(&serde_json::json!({
            "form": { "username": username, "password": "password" }
        }))
}

/// Signs `username` (password `"password"`) into an [`api_server`] cookie jar
/// through the login server fn, the way the Dioxus clients do.
///
/// Panics unless the login succeeds, so a test that gets past this is looking
/// at a signed-in session.
pub async fn login_as(server: &TestServer, username: &str) {
    login_request(server, username).await.assert_status_ok();
}
