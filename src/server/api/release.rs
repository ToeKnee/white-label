//! Release endpoints ported from `routes/release.rs` (see the `api` module
//! docs for the wire protocol). Permission checks stay in the services.

use axum::response::Response;

use crate::error::AppError;
use crate::models::{artist::Artist, record_label::RecordLabel};
use crate::routes::release::{
    CreateRelease, DeleteRelease, GetNextScheduledRelease, GetRelease, GetReleases, RestoreRelease,
    UpdateRelease,
};
use crate::server::api::{WireError, decode_args, json_response};
use crate::server::current_user::{CurrentUser, Db};
use crate::services::release::{
    create_release_service, delete_release_service, get_next_scheduled_release_service,
    get_release_service, get_releases_service, restore_release_service, update_release_service,
};

const PATH_GET_RELEASES: &str = "/api/get_releases";
const PATH_GET_NEXT_SCHEDULED_RELEASE: &str = "/api/get_next_scheduled_release";
const PATH_GET_RELEASE: &str = "/api/get_release";
const PATH_CREATE_RELEASE: &str = "/api/create_release";
const PATH_UPDATE_RELEASE: &str = "/api/update_release";
const PATH_DELETE_RELEASE: &str = "/api/delete_release";
const PATH_RESTORE_RELEASE: &str = "/api/restore_release";

/// List an artist's releases. Public — `label_owner` sessions also see
/// unpublished and deleted releases.
pub async fn get_releases(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<GetReleases>(PATH_GET_RELEASES, &body)?;

    let result = get_releases_service(&pool, current_user.as_ref(), args.artist_slug)
        .await
        .map_err(|error| WireError::from_app_error(PATH_GET_RELEASES, &error))?;

    json_response(PATH_GET_RELEASES, &result)
}

/// The next upcoming release (`release_date` after now). Public.
///
/// Quirk preserved from the Leptos route: the `artist_slug` filter is only
/// ever applied when the slug is the empty string (the match guard reads
/// `slug.is_empty()` where it presumably meant the opposite), so a real
/// slug never filters. Fixing it would change behaviour — deferred.
pub async fn get_next_scheduled_release(Db(pool): Db, body: String) -> Result<Response, WireError> {
    let args = decode_args::<GetNextScheduledRelease>(PATH_GET_NEXT_SCHEDULED_RELEASE, &body)?;

    let record_label = RecordLabel::first(&pool).await.map_err(|error| {
        tracing::error!("get_next_scheduled_release: {error:?}");
        WireError::from_app_error(
            PATH_GET_NEXT_SCHEDULED_RELEASE,
            &AppError::internal("Failed to retrieve record label"),
        )
    })?;
    let artist_id = match args.artist_slug {
        Some(slug) if slug.is_empty() => Artist::get_by_slug(&pool, slug)
            .await
            .ok()
            .map(|artist| artist.id),
        _ => None,
    };

    let result = get_next_scheduled_release_service(&pool, artist_id, record_label.id)
        .await
        .map_err(|error| WireError::from_app_error(PATH_GET_NEXT_SCHEDULED_RELEASE, &error))?;

    json_response(PATH_GET_NEXT_SCHEDULED_RELEASE, &result)
}

/// Fetch one release by slug with its artists and tracks. Public —
/// `label_owner` sessions also see unpublished and deleted releases.
pub async fn get_release(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<GetRelease>(PATH_GET_RELEASE, &body)?;

    let result = get_release_service(&pool, current_user.as_ref(), args.artist_slug, args.slug)
        .await
        .map_err(|error| WireError::from_app_error(PATH_GET_RELEASE, &error))?;

    json_response(PATH_GET_RELEASE, &result)
}

/// Create a release and attach its artists (admin + `label_owner`).
pub async fn create_release(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<CreateRelease>(PATH_CREATE_RELEASE, &body)?;

    let result = create_release_service(&pool, current_user.as_ref(), args.form)
        .await
        .map_err(|error| WireError::from_app_error(PATH_CREATE_RELEASE, &error))?;

    json_response(PATH_CREATE_RELEASE, &result)
}

/// Update an existing release (admin + `label_owner`).
pub async fn update_release(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<UpdateRelease>(PATH_UPDATE_RELEASE, &body)?;

    let result = update_release_service(&pool, current_user.as_ref(), args.form)
        .await
        .map_err(|error| WireError::from_app_error(PATH_UPDATE_RELEASE, &error))?;

    json_response(PATH_UPDATE_RELEASE, &result)
}

/// Soft-delete a release (admin + `label_owner`).
pub async fn delete_release(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<DeleteRelease>(PATH_DELETE_RELEASE, &body)?;

    let result = delete_release_service(&pool, current_user.as_ref(), args.slug)
        .await
        .map_err(|error| WireError::from_app_error(PATH_DELETE_RELEASE, &error))?;

    json_response(PATH_DELETE_RELEASE, &result)
}

/// Restore a soft-deleted release (admin + `label_owner`).
pub async fn restore_release(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<RestoreRelease>(PATH_RESTORE_RELEASE, &body)?;

    let result = restore_release_service(&pool, current_user.as_ref(), args.slug)
        .await
        .map_err(|error| WireError::from_app_error(PATH_RESTORE_RELEASE, &error))?;

    json_response(PATH_RESTORE_RELEASE, &result)
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::{
        PATH_CREATE_RELEASE, PATH_DELETE_RELEASE, PATH_GET_NEXT_SCHEDULED_RELEASE,
        PATH_GET_RELEASE, PATH_GET_RELEASES, PATH_RESTORE_RELEASE, PATH_UPDATE_RELEASE,
    };
    use crate::forms::release::{CreateReleaseForm, UpdateReleaseForm};
    use crate::models::test_helpers::{
        create_test_artist, create_test_record_label, create_test_release,
        create_test_user_with_permissions,
    };
    use crate::routes::auth::Login;
    use crate::routes::release::{
        CreateRelease, DeleteRelease, GetNextScheduledRelease, GetRelease, GetReleases,
        ReleaseResult, ReleasesResult, RestoreRelease, UpdateRelease,
    };
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
    async fn get_releases_returns_the_artist_releases(pool: PgPool) {
        let label = create_test_record_label(&pool, 1).await.expect("label");
        let artist = create_test_artist(&pool, 1, Some(label))
            .await
            .expect("artist");
        create_test_release(&pool, 1, Some(artist))
            .await
            .expect("release");
        let server = api_server(pool).await;

        let response = post_args(
            &server,
            PATH_GET_RELEASES,
            &GetReleases {
                artist_slug: "test-artist-1".to_string(),
            },
        )
        .await;

        response.assert_status_ok();
        let result: ReleasesResult = response.json();
        assert_eq!(result.releases.len(), 1);
        assert_eq!(result.releases[0].name, "Test Release 1");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn get_releases_missing_artist_is_internal(pool: PgPool) {
        let server = api_server(pool).await;

        let response = post_args(
            &server,
            PATH_GET_RELEASES,
            &GetReleases {
                artist_slug: "missing-artist".to_string(),
            },
        )
        .await;

        response.assert_status_internal_server_error();
        assert_eq!(response.header("serverfnerror"), PATH_GET_RELEASES);
        assert!(response.text().starts_with("ServerError|"));
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn get_release_returns_the_release_with_its_artists(pool: PgPool) {
        let label = create_test_record_label(&pool, 2).await.expect("label");
        let artist = create_test_artist(&pool, 2, Some(label))
            .await
            .expect("artist");
        create_test_release(&pool, 2, Some(artist))
            .await
            .expect("release");
        let server = api_server(pool).await;

        let response = post_args(
            &server,
            PATH_GET_RELEASE,
            &GetRelease {
                artist_slug: "test-artist-2".to_string(),
                slug: "test-release-2".to_string(),
            },
        )
        .await;

        response.assert_status_ok();
        let result: ReleaseResult = response.json();
        assert_eq!(result.release.slug, "test-release-2");
        assert_eq!(result.artists.len(), 1);
        assert_eq!(result.artists[0].slug, "test-artist-2");
        assert_eq!(result.tracks.len(), 0);
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn get_next_scheduled_release_without_label_is_internal(pool: PgPool) {
        let server = api_server(pool).await;

        let response = post_args(
            &server,
            PATH_GET_NEXT_SCHEDULED_RELEASE,
            &GetNextScheduledRelease { artist_slug: None },
        )
        .await;

        response.assert_status_internal_server_error();
        assert_eq!(
            response.text(),
            "ServerError|Failed to retrieve record label"
        );
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn get_next_scheduled_release_null_without_upcoming_release(pool: PgPool) {
        create_test_record_label(&pool, 3).await.expect("label");
        let server = api_server(pool).await;

        let response = post_args(
            &server,
            PATH_GET_NEXT_SCHEDULED_RELEASE,
            &GetNextScheduledRelease { artist_slug: None },
        )
        .await;

        response.assert_status_ok();
        assert_eq!(response.text(), "null");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn label_owner_release_crud_chain(pool: PgPool) {
        let label = create_test_record_label(&pool, 4).await.expect("label");
        let artist = create_test_artist(&pool, 4, Some(label.clone()))
            .await
            .expect("artist");
        create_test_user_with_permissions(&pool, 4, vec!["admin", "label_owner"])
            .await
            .expect("admin + label_owner user");
        let server = api_server(pool.clone()).await;

        post_args(&server, "/api/login", &login_args("test-4"))
            .await
            .assert_status_ok();

        // A release dated a week out doubles as the "next scheduled" one.
        let created = post_args(
            &server,
            PATH_CREATE_RELEASE,
            &CreateRelease {
                form: CreateReleaseForm {
                    artist_ids: artist.id.to_string(),
                    name: "Album One".to_string(),
                    description: "the debut".to_string(),
                    primary_artist_id: artist.id,
                    catalogue_number: "CAT-001".to_string(),
                    release_date: Some(chrono::Utc::now() + chrono::Duration::days(7)),
                    label_id: label.id,
                    published_at: Some(chrono::Utc::now()),
                },
            },
        )
        .await;
        created.assert_status_ok();
        let created: ReleaseResult = created.json();
        assert_eq!(created.release.slug, "album-one");
        assert_eq!(created.artists.len(), 1);

        let next = post_args(
            &server,
            PATH_GET_NEXT_SCHEDULED_RELEASE,
            &GetNextScheduledRelease {
                artist_slug: Some("test-artist-4".to_string()), // ignored — see quirk note
            },
        )
        .await;
        next.assert_status_ok();
        let next: Option<ReleaseResult> = next.json();
        assert_eq!(next.expect("next scheduled").release.slug, "album-one");

        let updated = post_args(
            &server,
            PATH_UPDATE_RELEASE,
            &UpdateRelease {
                form: UpdateReleaseForm {
                    artist_ids: artist.id.to_string(),
                    slug: "album-one".to_string(),
                    name: "Album One Deluxe".to_string(),
                    description: "expanded".to_string(),
                    primary_artist_id: artist.id,
                    catalogue_number: "CAT-001".to_string(),
                    release_date: Some(chrono::Utc::now() + chrono::Duration::days(7)),
                    label_id: label.id,
                    published_at: Some(chrono::Utc::now()),
                },
            },
        )
        .await;
        updated.assert_status_ok();
        let updated: ReleaseResult = updated.json();
        assert_eq!(updated.release.name, "Album One Deluxe");
        // `Release::update` re-slugifies from the name, like the other models.
        assert_eq!(updated.release.slug, "album-one-deluxe");

        let deleted = post_args(
            &server,
            PATH_DELETE_RELEASE,
            &DeleteRelease {
                slug: "album-one-deluxe".to_string(),
            },
        )
        .await;
        deleted.assert_status_ok();
        let deleted: ReleaseResult = deleted.json();
        assert!(deleted.release.deleted_at.is_some());

        let restored = post_args(
            &server,
            PATH_RESTORE_RELEASE,
            &RestoreRelease {
                slug: "album-one-deluxe".to_string(),
            },
        )
        .await;
        restored.assert_status_ok();
        let restored: ReleaseResult = restored.json();
        assert!(restored.release.deleted_at.is_none());
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn create_release_without_login_is_unauthorized(pool: PgPool) {
        let server = api_server(pool).await;

        let response = post_args(
            &server,
            PATH_CREATE_RELEASE,
            &CreateRelease {
                form: CreateReleaseForm {
                    artist_ids: "1".to_string(),
                    name: "Orphan Album".to_string(),
                    description: String::new(),
                    primary_artist_id: 1,
                    catalogue_number: "CAT-404".to_string(),
                    release_date: None,
                    label_id: 1,
                    published_at: None,
                },
            },
        )
        .await;

        response.assert_status_unauthorized();
        assert_eq!(response.text(), "ServerError|User not supplied.");
    }
}
