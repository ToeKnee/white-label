//! Artist link endpoints ported from `routes/links.rs` (see the `api`
//! module docs for the wire protocol). Permission checks stay in the
//! services.
//!
//! The `update_links` wire endpoint is retired: the dioxus
//! `#[post("/api/update_links")]` server fn in `ui/data.rs` owns that
//! exact path now (axum cannot register the same method+path twice), and
//! the links editor's reactive submit is the only caller. The Leptos-side
//! endpoint in the old server binary is unaffected.

use axum::response::Response;

use crate::routes::links::GetLinks;
use crate::server::api::{WireError, decode_args, json_response};
use crate::server::current_user::Db;
use crate::services::links::get_links_service;

const PATH_GET_LINKS: &str = "/api/get_links";

/// List an artist's music-service and social links. Public.
pub async fn get_links(Db(pool): Db, body: String) -> Result<Response, WireError> {
    let args = decode_args::<GetLinks>(PATH_GET_LINKS, &body)?;

    let result = get_links_service(&pool, args.artist_slug)
        .await
        .map_err(|error| WireError::from_app_error(PATH_GET_LINKS, &error))?;

    json_response(PATH_GET_LINKS, &result)
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::PATH_GET_LINKS;
    use crate::models::test_helpers::create_test_artist;
    use crate::routes::links::{GetLinks, LinksResult};
    use crate::server::api::test_support::{api_server, post_args};

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn get_links_returns_empty_lists_for_artist_without_links(pool: PgPool) {
        create_test_artist(&pool, 1, None).await.expect("artist");
        let server = api_server(pool).await;

        let response = post_args(
            &server,
            PATH_GET_LINKS,
            &GetLinks {
                artist_slug: "test-artist-1".to_string(),
            },
        )
        .await;

        response.assert_status_ok();
        let result: LinksResult = response.json();
        assert_eq!(result.music_services.len(), 0);
        assert_eq!(result.social_media_services.len(), 0);
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn get_links_missing_artist_is_not_found(pool: PgPool) {
        let server = api_server(pool).await;

        let response = post_args(
            &server,
            PATH_GET_LINKS,
            &GetLinks {
                artist_slug: "missing-artist".to_string(),
            },
        )
        .await;

        response.assert_status_not_found();
        assert_eq!(response.header("serverfnerror"), PATH_GET_LINKS);
        assert!(response.text().starts_with("ServerError|Artist not found:"));
    }
}
