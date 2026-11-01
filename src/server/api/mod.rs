//! Dioxus-side `/api/*` endpoints (MIGRATION.md Phase 2).
//!
//! These speak the exact wire protocol of the Leptos 0.8 `#[server]`
//! functions they replace (`Http<PostUrl, Json>` in `server_fn` 0.8), so the
//! shipped Leptos UI can call this server unchanged during the overlap:
//!
//! - Request: `POST /api/<endpoint>` with a form-urlencoded body — the
//!   `serde_qs` encoding of the `#[server]`-generated args struct
//!   (e.g. `GetArtist { slug }` → `slug=my-artist`). Requests are decoded
//!   with the same `serde_qs::Config::new(5, false)` the `PostUrl` codec uses.
//! - Success: `200 OK`, `application/json`, body is the JSON of the `Ok`
//!   value.
//! - Error: the status now comes from [`AppError::status`] (the Leptos
//!   handlers answer 500 for everything, so this is a deliberate upgrade),
//!   while the body stays `ServerError|<message>` with the `serverfnerror`
//!   header — the bytes the Leptos client parses and displays today.
//!
//! Handlers reuse the args structs from `crate::routes` (the `#[server]`
//! macro emits them regardless of features), so the two servers cannot
//! drift apart. Tests encode requests with `serde_qs::to_string`, which is
//! literally what the Leptos client does.

mod artist;
mod auth;
mod links;
mod menu;
mod page;
mod record_label;
mod track;

use axum::Router;
use axum::response::{IntoResponse, Response};
use axum::routing::post;

use crate::error::AppError;

/// Assemble the ported `/api/*` routes. `build_router` merges these under
/// the session and `Db` layers, so handlers can extract
/// [`tower_sessions::Session`], [`super::current_user::CurrentUser`] and
/// [`super::current_user::Db`].
pub fn api_routes() -> Router {
    Router::new()
        // Auth (routes/auth.rs)
        .route("/api/user", post(auth::get_user))
        .route("/api/login", post(auth::login))
        .route("/api/register", post(auth::register))
        .route("/api/logout", post(auth::logout))
        .route("/api/update_profile", post(auth::update_user))
        .route("/api/change_password", post(auth::change_password))
        // Artists (routes/artist.rs, routes/artists.rs)
        .route("/api/get_artist", post(artist::get_artist))
        .route("/api/create_artist", post(artist::create_artist))
        .route("/api/update_artist", post(artist::update_artist))
        .route("/api/delete_artist", post(artist::delete_artist))
        .route("/api/restore_artist", post(artist::restore_artist))
        .route("/api/artists_releases", post(artist::artists_releases))
        // Pages (routes/page.rs)
        .route("/api/get_page", post(page::get_page))
        .route("/api/create_page", post(page::create_page))
        .route("/api/update_page", post(page::update_page))
        .route("/api/delete_page", post(page::delete_page))
        .route("/api/restore_page", post(page::restore_page))
        // Record label (routes/record_label.rs)
        .route("/api/record_label", post(record_label::get_record_label))
        .route(
            "/api/record_label_artists",
            post(record_label::get_label_artists),
        )
        .route(
            "/api/record_label_pages",
            post(record_label::get_label_pages),
        )
        .route(
            "/api/update_record_label",
            post(record_label::update_record_label),
        )
        // Admin menu (routes/menu.rs)
        .route("/api/admin_menu", post(menu::admin_menu))
        // Artist links (routes/links.rs)
        .route("/api/get_links", post(links::get_links))
        .route("/api/update_links", post(links::update_links))
        // Tracks (routes/track.rs)
        .route("/api/get_tracks", post(track::get_tracks))
        .route("/api/get_track", post(track::get_track))
        .route("/api/create_track", post(track::create_track))
        .route("/api/update_track", post(track::update_track))
        .route("/api/delete_track", post(track::delete_track))
        .route("/api/restore_track", post(track::restore_track))
}

/// A failure rendered in the Leptos server-fn error wire format:
/// `"<Variant>|<message>"`, plus the `serverfnerror: <endpoint>` header the
/// Leptos handlers set.
pub struct WireError {
    endpoint: &'static str,
    status: axum::http::StatusCode,
    body: String,
}

impl WireError {
    /// Service-level failure: status from [`AppError::status`], body kept as
    /// the `ServerError|<message>` the Leptos client shows today.
    pub fn from_app_error(endpoint: &'static str, error: &AppError) -> Self {
        Self {
            endpoint,
            status: axum::http::StatusCode::from_u16(error.status())
                .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
            body: format!("ServerError|{error}"),
        }
    }

    /// Request body failed to decode. Same `Args|` variant the `PostUrl`
    /// codec emits today (again with a more accurate 400 status).
    fn args(endpoint: &'static str, detail: impl std::fmt::Display) -> Self {
        Self {
            endpoint,
            status: axum::http::StatusCode::BAD_REQUEST,
            body: format!("Args|{detail}"),
        }
    }

    /// Server-side failure while serving (500, matching current behaviour).
    fn internal(endpoint: &'static str, detail: impl std::fmt::Display) -> Self {
        Self {
            endpoint,
            status: axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            body: format!("ServerError|{detail}"),
        }
    }
}

impl IntoResponse for WireError {
    fn into_response(self) -> Response {
        (
            self.status,
            [(server_fn::error::SERVER_FN_ERROR_HEADER, self.endpoint)],
            self.body,
        )
            .into_response()
    }
}

/// Decode a server-fn request body exactly as the Leptos `PostUrl` codec does.
pub fn decode_args<T>(endpoint: &'static str, body: &str) -> Result<T, WireError>
where
    T: serde::de::DeserializeOwned,
{
    serde_qs::Config::new(5, false)
        .deserialize_str::<T>(body)
        .map_err(|error| WireError::args(endpoint, error))
}

/// Success response matching the Leptos `Json` output encoding.
pub fn json_response<T>(endpoint: &'static str, value: &T) -> Result<Response, WireError>
where
    T: serde::Serialize,
{
    let body = serde_json::to_vec(value)
        .map_err(|error| WireError::internal(endpoint, format!("could not serialize: {error}")))?;
    Ok((
        [(
            axum::http::header::CONTENT_TYPE,
            axum::http::header::HeaderValue::from_static("application/json"),
        )],
        axum::body::Body::from(body),
    )
        .into_response())
}

#[cfg(test)]
pub mod test_support {
    use axum_test::{TestRequest, TestServer, TestServerConfig};
    use sqlx::PgPool;

    use crate::server::build_router;

    /// Build the full router (SSR root + API routes) for wire tests.
    /// `save_cookies` makes `TestServer` replay `Set-Cookie` between
    /// requests, i.e. it behaves like the browser the Leptos UI runs in.
    pub async fn api_server(pool: PgPool) -> TestServer {
        crate::server::ensure_exe_public_dir();
        let router = build_router(pool, None)
            .await
            .expect("router should build against the test pool");
        let config = TestServerConfig {
            save_cookies: true,
            ..TestServerConfig::default()
        };
        TestServer::new_with_config(router, config).expect("axum-test should wrap the router")
    }

    /// `POST` with the body the Leptos client would send for `args`:
    /// `serde_qs::to_string` of the server-fn args struct, form-urlencoded.
    /// `TestServer` carries the session cookie on later requests.
    pub fn post_args<T: serde::Serialize>(
        server: &TestServer,
        path: &str,
        args: &T,
    ) -> TestRequest {
        let body = serde_qs::to_string(args).expect("server-fn args should serialize");
        server
            .post(path)
            .content_type("application/x-www-form-urlencoded")
            .bytes(axum::body::Bytes::from(body))
    }
}
