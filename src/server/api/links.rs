//! Artist link endpoints ported from `routes/links.rs` (see the `api` module
//! docs for the wire protocol). Permission checks stay in the services.

use axum::response::Response;

use crate::routes::links::{GetLinks, UpdateLinks};
use crate::server::api::{WireError, decode_args, json_response};
use crate::server::current_user::{CurrentUser, Db};
use crate::services::links::{get_links_service, update_links_service};

const PATH_GET_LINKS: &str = "/api/get_links";
const PATH_UPDATE_LINKS: &str = "/api/update_links";

/// List an artist's music-service and social links. Public.
pub async fn get_links(Db(pool): Db, body: String) -> Result<Response, WireError> {
    let args = decode_args::<GetLinks>(PATH_GET_LINKS, &body)?;

    let result = get_links_service(&pool, args.artist_slug)
        .await
        .map_err(|error| WireError::from_app_error(PATH_GET_LINKS, &error))?;

    json_response(PATH_GET_LINKS, &result)
}

/// Replace an artist's links from the flat `LinksForm`
/// (admin + `label_owner` required by the service).
pub async fn update_links(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<UpdateLinks>(PATH_UPDATE_LINKS, &body)?;

    let result = update_links_service(&pool, current_user.as_ref(), args.form)
        .await
        .map_err(|error| WireError::from_app_error(PATH_UPDATE_LINKS, &error))?;

    json_response(PATH_UPDATE_LINKS, &result)
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::{PATH_GET_LINKS, PATH_UPDATE_LINKS};
    use crate::forms::links::LinksForm;
    use crate::models::music_service::Platform;
    use crate::models::social_media::SocialMedia;
    use crate::models::test_helpers::{create_test_artist, create_test_user_with_permissions};
    use crate::routes::auth::Login;
    use crate::routes::links::{GetLinks, LinksResult, UpdateLinks};
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

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn update_links_without_login_is_unauthorized(pool: PgPool) {
        let server = api_server(pool).await;

        let response = post_args(
            &server,
            PATH_UPDATE_LINKS,
            &UpdateLinks {
                form: LinksForm {
                    artist_slug: "test-artist-2".to_string(),
                    ..Default::default()
                },
            },
        )
        .await;

        response.assert_status_unauthorized();
        assert_eq!(response.text(), "ServerError|User not supplied.");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn label_owner_can_create_and_remove_links(pool: PgPool) {
        create_test_artist(&pool, 3, None).await.expect("artist");
        create_test_user_with_permissions(&pool, 3, vec!["admin", "label_owner"])
            .await
            .expect("admin + label_owner user");
        let server = api_server(pool).await;

        post_args(
            &server,
            "/api/login",
            &Login {
                username: "test-3".to_string(),
                password: "password".to_string(),
                remember: None,
            },
        )
        .await
        .assert_status_ok();

        let created = post_args(
            &server,
            PATH_UPDATE_LINKS,
            &UpdateLinks {
                form: LinksForm {
                    artist_slug: "test-artist-3".to_string(),
                    spotify: "https://open.spotify.example/artist".to_string(),
                    facebook: "https://facebook.example/artist".to_string(),
                    ..Default::default()
                },
            },
        )
        .await;
        created.assert_status_ok();
        let created: LinksResult = created.json();
        assert_eq!(created.music_services.len(), 1);
        assert_eq!(created.music_services[0].platform, Platform::Spotify);
        assert_eq!(
            created.music_services[0].url,
            "https://open.spotify.example/artist"
        );
        assert_eq!(created.social_media_services.len(), 1);
        assert_eq!(
            created.social_media_services[0].platform,
            SocialMedia::Facebook
        );

        // Re-sending the form updates the spotify url in place and deletes
        // the facebook link (the service diffs against what's in the DB).
        let updated = post_args(
            &server,
            PATH_UPDATE_LINKS,
            &UpdateLinks {
                form: LinksForm {
                    artist_slug: "test-artist-3".to_string(),
                    spotify: "https://open.spotify.example/changed".to_string(),
                    ..Default::default()
                },
            },
        )
        .await;
        updated.assert_status_ok();
        let updated: LinksResult = updated.json();
        assert_eq!(updated.music_services.len(), 1);
        assert_eq!(
            updated.music_services[0].url,
            "https://open.spotify.example/changed"
        );
        assert_eq!(updated.social_media_services.len(), 0);
    }
}
