//! Artist endpoints ported from `routes/artist.rs` and `routes/artists.rs`
//! (see the `api` module docs for the wire protocol). Permission checks
//! stay in the services, which return [`AppError`]s that map straight onto
//! wire statuses.

use axum::response::Response;

use crate::routes::artist::{CreateArtist, DeleteArtist, GetArtist, RestoreArtist, UpdateArtist};
use crate::routes::artists::{ArtistsReleaseResult, GetArtistsReleases};
use crate::server::api::{WireError, decode_args, json_response};
use crate::server::current_user::{CurrentUser, Db};
use crate::services::artist::{
    create_artist_service, delete_artist_service, get_artist_service, restore_artist_service,
    update_artist_service,
};
use crate::services::artists::get_releases_for_artist_service;

const PATH_GET_ARTIST: &str = "/api/get_artist";
const PATH_CREATE_ARTIST: &str = "/api/create_artist";
const PATH_UPDATE_ARTIST: &str = "/api/update_artist";
const PATH_DELETE_ARTIST: &str = "/api/delete_artist";
const PATH_RESTORE_ARTIST: &str = "/api/restore_artist";
const PATH_ARTISTS_RELEASES: &str = "/api/artists_releases";

/// Get a single artist by slug. Public — no session needed.
pub async fn get_artist(Db(pool): Db, body: String) -> Result<Response, WireError> {
    let args = decode_args::<GetArtist>(PATH_GET_ARTIST, &body)?;

    let result = get_artist_service(&pool, args.slug)
        .await
        .map_err(|error| WireError::from_app_error(PATH_GET_ARTIST, &error))?;

    json_response(PATH_GET_ARTIST, &result)
}

/// Create a new artist (admin / `label_owner`).
pub async fn create_artist(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<CreateArtist>(PATH_CREATE_ARTIST, &body)?;

    let result = create_artist_service(&pool, current_user.as_ref(), args.artist_form)
        .await
        .map_err(|error| WireError::from_app_error(PATH_CREATE_ARTIST, &error))?;

    json_response(PATH_CREATE_ARTIST, &result)
}

/// Update an existing artist (admin / `label_owner`).
pub async fn update_artist(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<UpdateArtist>(PATH_UPDATE_ARTIST, &body)?;

    let result = update_artist_service(&pool, current_user.as_ref(), args.artist_form)
        .await
        .map_err(|error| WireError::from_app_error(PATH_UPDATE_ARTIST, &error))?;

    json_response(PATH_UPDATE_ARTIST, &result)
}

/// Soft-delete an artist (admin / `label_owner`).
pub async fn delete_artist(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<DeleteArtist>(PATH_DELETE_ARTIST, &body)?;

    let result = delete_artist_service(&pool, current_user.as_ref(), args.slug)
        .await
        .map_err(|error| WireError::from_app_error(PATH_DELETE_ARTIST, &error))?;

    json_response(PATH_DELETE_ARTIST, &result)
}

/// Restore a soft-deleted artist (admin / `label_owner`).
pub async fn restore_artist(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<RestoreArtist>(PATH_RESTORE_ARTIST, &body)?;

    let result = restore_artist_service(&pool, current_user.as_ref(), args.slug)
        .await
        .map_err(|error| WireError::from_app_error(PATH_RESTORE_ARTIST, &error))?;

    json_response(PATH_RESTORE_ARTIST, &result)
}

/// Get releases for one artist (admin / `label_owner`).
///
/// Note: the Leptos route flattened every service failure into "Could not
/// retrieve releases, try again later". Here auth failures keep their own
/// messages, matching every other artist endpoint; only unexpected
/// failures collapse to a generic 500 via `AppError::Internal`.
pub async fn artists_releases(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<GetArtistsReleases>(PATH_ARTISTS_RELEASES, &body)?;

    let releases = get_releases_for_artist_service(&pool, current_user.as_ref(), args.artist_id)
        .await
        .map_err(|error| {
            tracing::error!("artists_releases: {error}");
            WireError::from_app_error(PATH_ARTISTS_RELEASES, &error)
        })?;

    json_response(PATH_ARTISTS_RELEASES, &ArtistsReleaseResult { releases })
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::{
        PATH_ARTISTS_RELEASES, PATH_CREATE_ARTIST, PATH_DELETE_ARTIST, PATH_GET_ARTIST,
        PATH_RESTORE_ARTIST,
    };
    use crate::forms::artist::{CreateArtistForm, UpdateArtistForm};
    use crate::models::test_helpers::{
        create_test_artist, create_test_record_label, create_test_release,
        create_test_user_with_permissions,
    };
    use crate::routes::artist::{
        ArtistResult, CreateArtist, DeleteArtist, GetArtist, RestoreArtist, UpdateArtist,
    };
    use crate::routes::artists::{ArtistsReleaseResult, GetArtistsReleases};
    use crate::routes::auth::Login;
    use crate::server::api::test_support::{api_server, post_args};

    /// Log the named test user (password "password") in through the API.
    fn login_args(username: &str) -> Login {
        Login {
            username: username.to_string(),
            password: "password".to_string(),
            remember: None,
        }
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn get_artist_returns_the_artist_json(pool: PgPool) {
        create_test_artist(&pool, 1, None)
            .await
            .expect("test artist");
        let server = api_server(pool).await;

        let response = post_args(
            &server,
            PATH_GET_ARTIST,
            &GetArtist {
                slug: "test-artist-1".to_string(),
            },
        )
        .await;

        response.assert_status_ok();
        let result: ArtistResult = response.json();
        assert_eq!(result.artist.name, "Test Artist 1");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn get_artist_missing_is_internal(pool: PgPool) {
        // Today's behaviour: the model's not-found error surfaces as a 500
        // `ServerError|…`. Refining this to a 404 NotFound is follow-up.
        let server = api_server(pool).await;

        let response = post_args(
            &server,
            PATH_GET_ARTIST,
            &GetArtist {
                slug: "missing-artist".to_string(),
            },
        )
        .await;

        response.assert_status_internal_server_error();
        assert_eq!(response.header("serverfnerror"), PATH_GET_ARTIST);
        assert!(response.text().starts_with("ServerError|"));
        assert!(response.text().contains("missing-artist"));
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn create_artist_without_login_is_unauthorized(pool: PgPool) {
        let server = api_server(pool).await;

        let response = post_args(
            &server,
            PATH_CREATE_ARTIST,
            &CreateArtist {
                artist_form: CreateArtistForm {
                    name: "Unsigned Artist".to_string(),
                    description: String::new(),
                    website: String::new(),
                    label_id: 1,
                    published_at: None,
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
    async fn create_artist_without_permission_is_forbidden(pool: PgPool) {
        create_test_user_with_permissions(&pool, 2, vec![])
            .await
            .expect("permissionless user");
        let server = api_server(pool).await;

        post_args(&server, "/api/login", &login_args("test-2"))
            .await
            .assert_status_ok();

        let response = post_args(
            &server,
            PATH_CREATE_ARTIST,
            &CreateArtist {
                artist_form: CreateArtistForm {
                    name: "Unpermitted Artist".to_string(),
                    description: String::new(),
                    website: String::new(),
                    label_id: 1,
                    published_at: None,
                },
            },
        )
        .await;

        response.assert_status_forbidden();
        assert_eq!(response.text(), "ServerError|You do not have permission.");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn label_owner_can_create_update_delete_and_restore(pool: PgPool) {
        let label = create_test_record_label(&pool, 3).await.expect("label");
        // The artist services require both tokens (user_with_permissions
        // checks every listed permission) — same as the service tests.
        create_test_user_with_permissions(&pool, 3, vec!["admin", "label_owner"])
            .await
            .expect("admin + label_owner user");
        let server = api_server(pool).await;

        post_args(&server, "/api/login", &login_args("test-3"))
            .await
            .assert_status_ok();

        let created = post_args(
            &server,
            PATH_CREATE_ARTIST,
            &CreateArtist {
                artist_form: CreateArtistForm {
                    name: "Signed Artist".to_string(),
                    description: "label owner signed them".to_string(),
                    website: "https://signed.example.com".to_string(),
                    label_id: label.id,
                    published_at: None,
                },
            },
        )
        .await;
        created.assert_status_ok();
        let created: ArtistResult = created.json();
        assert_eq!(created.artist.slug, "signed-artist");
        assert_eq!(created.artist.label_id, label.id);

        let updated = post_args(
            &server,
            "/api/update_artist",
            &UpdateArtist {
                artist_form: UpdateArtistForm {
                    slug: "signed-artist".to_string(),
                    name: "Signed Artist Renamed".to_string(),
                    description: "still here".to_string(),
                    website: "https://signed.example.com".to_string(),
                    published_at: None,
                },
            },
        )
        .await;
        updated.assert_status_ok();
        let updated: ArtistResult = updated.json();
        assert_eq!(updated.artist.name, "Signed Artist Renamed");
        // `Artist::update` re-slugifies from the name (existing model
        // behaviour), so later calls address the new slug.
        assert_eq!(updated.artist.slug, "signed-artist-renamed");

        post_args(
            &server,
            PATH_DELETE_ARTIST,
            &DeleteArtist {
                slug: "signed-artist-renamed".to_string(),
            },
        )
        .await
        .assert_status_ok();

        post_args(
            &server,
            PATH_RESTORE_ARTIST,
            &RestoreArtist {
                slug: "signed-artist-renamed".to_string(),
            },
        )
        .await
        .assert_status_ok();

        let fetched = post_args(
            &server,
            PATH_GET_ARTIST,
            &GetArtist {
                slug: "signed-artist-renamed".to_string(),
            },
        )
        .await;
        fetched.assert_status_ok();
        let fetched: ArtistResult = fetched.json();
        assert_eq!(fetched.artist.name, "Signed Artist Renamed");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn artists_releases_requires_permission_and_returns_releases(pool: PgPool) {
        let artist = create_test_artist(&pool, 5, None).await.expect("artist");
        create_test_release(&pool, 5, Some(artist.clone()))
            .await
            .expect("release");
        create_test_user_with_permissions(&pool, 5, vec!["admin", "label_owner"])
            .await
            .expect("admin + label_owner user");
        let server = api_server(pool).await;

        // Anonymous first: the permission check answers 401 before the DB work.
        let anonymous = post_args(
            &server,
            PATH_ARTISTS_RELEASES,
            &GetArtistsReleases {
                artist_id: artist.id,
            },
        )
        .await;
        anonymous.assert_status_unauthorized();
        assert_eq!(anonymous.text(), "ServerError|User not supplied.");

        post_args(&server, "/api/login", &login_args("test-5"))
            .await
            .assert_status_ok();

        let response = post_args(
            &server,
            PATH_ARTISTS_RELEASES,
            &GetArtistsReleases {
                artist_id: artist.id,
            },
        )
        .await;

        response.assert_status_ok();
        let releases: ArtistsReleaseResult = response.json();
        assert_eq!(releases.releases.len(), 1);
        assert_eq!(releases.releases[0].slug, "test-release-5");
    }
}
