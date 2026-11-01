//! Track endpoints ported from `routes/track.rs` (see the `api` module docs
//! for the wire protocol). Permission checks stay in the services.
//!
//! Note: `get_tracks` is public (only published tracks), but
//! `get_track_service` requires a logged-in user — preserved as-is.

use axum::response::Response;

use crate::routes::track::{
    CreateTrack, DeleteTrack, GetTrack, GetTracks, RestoreTrack, UpdateTrack,
};
use crate::server::api::{WireError, decode_args, json_response};
use crate::server::current_user::{CurrentUser, Db};
use crate::services::track::{
    create_track_service, delete_track_service, get_track_service, get_tracks_service,
    restore_track_service, update_track_service,
};

const PATH_GET_TRACKS: &str = "/api/get_tracks";
const PATH_GET_TRACK: &str = "/api/get_track";
const PATH_CREATE_TRACK: &str = "/api/create_track";
const PATH_UPDATE_TRACK: &str = "/api/update_track";
const PATH_DELETE_TRACK: &str = "/api/delete_track";
const PATH_RESTORE_TRACK: &str = "/api/restore_track";

/// List an artist's tracks for one release. Public — `label_owner`
/// sessions also see unpublished and deleted tracks.
pub async fn get_tracks(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<GetTracks>(PATH_GET_TRACKS, &body)?;

    let result = get_tracks_service(
        &pool,
        current_user.as_ref(),
        args.artist_slug,
        args.release_slug,
    )
    .await
    .map_err(|error| WireError::from_app_error(PATH_GET_TRACKS, &error))?;

    json_response(PATH_GET_TRACKS, &result)
}

/// Fetch one track by slug with its artists. Requires a session (the
/// service answers 401 otherwise, matching the Leptos route).
pub async fn get_track(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<GetTrack>(PATH_GET_TRACK, &body)?;

    let result = get_track_service(
        &pool,
        current_user.as_ref(),
        args.artist_slug,
        args.release_slug,
        args.slug,
    )
    .await
    .map_err(|error| WireError::from_app_error(PATH_GET_TRACK, &error))?;

    json_response(PATH_GET_TRACK, &result)
}

/// Create a track and attach its artists (admin + `label_owner`).
pub async fn create_track(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<CreateTrack>(PATH_CREATE_TRACK, &body)?;

    let result = create_track_service(&pool, current_user.as_ref(), args.form)
        .await
        .map_err(|error| WireError::from_app_error(PATH_CREATE_TRACK, &error))?;

    json_response(PATH_CREATE_TRACK, &result)
}

/// Update an existing track (admin + `label_owner`).
pub async fn update_track(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<UpdateTrack>(PATH_UPDATE_TRACK, &body)?;

    let result = update_track_service(&pool, current_user.as_ref(), args.form)
        .await
        .map_err(|error| WireError::from_app_error(PATH_UPDATE_TRACK, &error))?;

    json_response(PATH_UPDATE_TRACK, &result)
}

/// Soft-delete a track (admin + `label_owner`).
pub async fn delete_track(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<DeleteTrack>(PATH_DELETE_TRACK, &body)?;

    let result = delete_track_service(&pool, current_user.as_ref(), args.slug)
        .await
        .map_err(|error| WireError::from_app_error(PATH_DELETE_TRACK, &error))?;

    json_response(PATH_DELETE_TRACK, &result)
}

/// Restore a soft-deleted track (admin + `label_owner`).
pub async fn restore_track(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<RestoreTrack>(PATH_RESTORE_TRACK, &body)?;

    let result = restore_track_service(&pool, current_user.as_ref(), args.slug)
        .await
        .map_err(|error| WireError::from_app_error(PATH_RESTORE_TRACK, &error))?;

    json_response(PATH_RESTORE_TRACK, &result)
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::{
        PATH_CREATE_TRACK, PATH_DELETE_TRACK, PATH_GET_TRACK, PATH_GET_TRACKS, PATH_RESTORE_TRACK,
        PATH_UPDATE_TRACK,
    };
    use crate::forms::track::{CreateTrackForm, UpdateTrackForm};
    use crate::models::test_helpers::{
        create_test_artist, create_test_record_label, create_test_release,
        create_test_user_with_permissions,
    };
    use crate::routes::auth::Login;
    use crate::routes::track::{
        CreateTrack, DeleteTrack, GetTrack, GetTracks, RestoreTrack, TrackResult, TracksResult,
        UpdateTrack,
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

    /// Fixture: label + artist + release, returned in that order.
    async fn release_fixture(pool: &PgPool, id: usize) -> (i64, i64, i64) {
        let label = create_test_record_label(pool, id).await.expect("label");
        let artist = create_test_artist(pool, id, Some(label.clone()))
            .await
            .expect("artist");
        let release = create_test_release(pool, id, Some(artist.clone()))
            .await
            .expect("release");
        (artist.id, release.id, label.id)
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn get_tracks_returns_empty_for_release_without_tracks(pool: PgPool) {
        let (_, _, _) = release_fixture(&pool, 1).await;
        let server = api_server(pool).await;

        let response = post_args(
            &server,
            PATH_GET_TRACKS,
            &GetTracks {
                artist_slug: "test-artist-1".to_string(),
                release_slug: "test-release-1".to_string(),
            },
        )
        .await;

        response.assert_status_ok();
        let result: TracksResult = response.json();
        assert_eq!(result.tracks.len(), 0);
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn get_tracks_missing_artist_is_not_found(pool: PgPool) {
        let server = api_server(pool).await;

        let response = post_args(
            &server,
            PATH_GET_TRACKS,
            &GetTracks {
                artist_slug: "missing-artist".to_string(),
                release_slug: "test-release-1".to_string(),
            },
        )
        .await;

        response.assert_status_not_found();
        assert_eq!(response.header("serverfnerror"), PATH_GET_TRACKS);
        assert!(response.text().starts_with("ServerError|"));
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn create_track_without_login_is_unauthorized(pool: PgPool) {
        let server = api_server(pool).await;

        let response = post_args(
            &server,
            PATH_CREATE_TRACK,
            &CreateTrack {
                form: CreateTrackForm {
                    artist_ids: "1".to_string(),
                    name: "Orphan Track".to_string(),
                    description: String::new(),
                    lyrics: String::new(),
                    primary_artist_id: 1,
                    release_id: 1,
                    isrc_code: None,
                    bpm: None,
                    track_number: 1,
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
    async fn get_track_without_login_is_unauthorized(pool: PgPool) {
        // `get_track_service` loads artist and release first, then rejects
        // with its own "User not found" message (note: not "User not
        // supplied." — the message differs from the permission helper).
        release_fixture(&pool, 2).await;
        let server = api_server(pool).await;

        let response = post_args(
            &server,
            PATH_GET_TRACK,
            &GetTrack {
                artist_slug: "test-artist-2".to_string(),
                release_slug: "test-release-2".to_string(),
                slug: "any-track".to_string(),
            },
        )
        .await;

        response.assert_status_unauthorized();
        assert_eq!(response.text(), "ServerError|User not found");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn label_owner_track_crud_chain(pool: PgPool) {
        let (artist_id, release_id, _) = release_fixture(&pool, 3).await;
        create_test_user_with_permissions(&pool, 3, vec!["admin", "label_owner"])
            .await
            .expect("admin + label_owner user");
        let server = api_server(pool.clone()).await;

        post_args(&server, "/api/login", &login_args("test-3"))
            .await
            .assert_status_ok();

        let created = post_args(
            &server,
            PATH_CREATE_TRACK,
            &CreateTrack {
                form: CreateTrackForm {
                    artist_ids: artist_id.to_string(),
                    name: "Song One".to_string(),
                    description: "debut".to_string(),
                    lyrics: "la la".to_string(),
                    primary_artist_id: artist_id,
                    release_id,
                    isrc_code: Some("USABC2600001".to_string()),
                    bpm: Some(120),
                    track_number: 1,
                    published_at: Some(chrono::Utc::now()),
                },
            },
        )
        .await;
        created.assert_status_ok();
        let created: TrackResult = created.json();
        assert_eq!(created.track.slug, "song-one");
        assert_eq!(created.artists.len(), 1);
        assert_eq!(created.artists[0].id, artist_id);

        // Published, so an anonymous session sees it in the release list.
        let anonymous = api_server(pool).await;
        let list = post_args(
            &anonymous,
            PATH_GET_TRACKS,
            &GetTracks {
                artist_slug: "test-artist-3".to_string(),
                release_slug: "test-release-3".to_string(),
            },
        )
        .await;
        list.assert_status_ok();
        let list: TracksResult = list.json();
        assert_eq!(list.tracks.len(), 1);
        assert_eq!(list.tracks[0].name, "Song One");

        let updated = post_args(
            &server,
            PATH_UPDATE_TRACK,
            &UpdateTrack {
                form: UpdateTrackForm {
                    artist_ids: artist_id.to_string(),
                    slug: "song-one".to_string(),
                    name: "Song One (Remaster)".to_string(),
                    description: "remastered".to_string(),
                    lyrics: "la la".to_string(),
                    primary_artist_id: artist_id,
                    release_id,
                    // `tracks.isrc_code` is NOT NULL, so sending None here
                    // 500s at the database — same in the Leptos app today.
                    isrc_code: Some("USABC2600001".to_string()),
                    bpm: None,
                    track_number: 2,
                    published_at: Some(chrono::Utc::now()),
                },
            },
        )
        .await;
        updated.assert_status_ok();
        let updated: TrackResult = updated.json();
        assert_eq!(updated.track.name, "Song One (Remaster)");
        // `Track::update` re-slugifies from the name, like the other models.
        assert_eq!(updated.track.slug, "song-one-remaster");

        // get_track works for the logged-in owner with the new slug.
        let fetched = post_args(
            &server,
            PATH_GET_TRACK,
            &GetTrack {
                artist_slug: "test-artist-3".to_string(),
                release_slug: "test-release-3".to_string(),
                slug: "song-one-remaster".to_string(),
            },
        )
        .await;
        fetched.assert_status_ok();
        let fetched: TrackResult = fetched.json();
        assert_eq!(fetched.track.track_number, 2);

        let deleted = post_args(
            &server,
            PATH_DELETE_TRACK,
            &DeleteTrack {
                slug: "song-one-remaster".to_string(),
            },
        )
        .await;
        deleted.assert_status_ok();
        let deleted: TrackResult = deleted.json();
        assert!(deleted.track.deleted_at.is_some());

        let restored = post_args(
            &server,
            PATH_RESTORE_TRACK,
            &RestoreTrack {
                slug: "song-one-remaster".to_string(),
            },
        )
        .await;
        restored.assert_status_ok();
        let restored: TrackResult = restored.json();
        assert!(restored.track.deleted_at.is_none());
    }
}
