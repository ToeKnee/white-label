//! Services for the tracks routes
use leptos::prelude::ServerFnError;
use sqlx::PgPool;

use super::authentication_helpers::user_with_permissions;
use crate::forms::track::{CreateTrackForm, UpdateTrackForm};
use crate::models::{artist::Artist, auth::User, release::Release, track::Track};
use crate::routes::track::{TrackResult, TracksResult};

/// Get an artists tracks
///
/// # Arguments
/// * `pool` - The database connection pool
/// * `user` - The current user
/// * `artist_slug` - The slug of the artist
/// * `release_slug` - The slug of the release
///
/// # Returns
/// The tracks for the artist
///
/// # Errors
/// If the artist cannot be found, return an error
/// If the release cannot be found, return an error
/// If the user cannot be found, return an error
/// If the tracks cannot be found, return an error
#[cfg(feature = "ssr")]
pub async fn get_tracks_service(
    pool: &PgPool,
    user: Option<&User>,
    artist_slug: String,
    release_slug: String,
) -> Result<TracksResult, ServerFnError> {
    let artist = match Artist::get_by_slug(pool, artist_slug).await {
        Ok(artist) => artist,
        Err(e) => {
            let err = format!("Error while getting artist: {e:?}");
            tracing::error!("{err}");
            return Err(ServerFnError::new(e));
        }
    };
    let release = match Release::get_by_slug(pool, release_slug).await {
        Ok(release) => release,
        Err(e) => {
            let err = format!("Error while getting release: {e:?}");
            tracing::error!("{err}");
            return Err(ServerFnError::new(e));
        }
    };

    let include_hidden =
        user.is_some_and(|current_user| current_user.permissions.contains("label_owner"));

    Ok(TracksResult {
        tracks: match Track::list_by_release_and_artist_and_record_label(
            pool,
            release.id,
            artist.id,
            artist.label_id,
            include_hidden,
        )
        .await
        {
            Ok(tracks) => tracks,
            Err(e) => {
                let err = format!("Error while getting tracks: {e:?}");
                tracing::error!("{err}");
                return Err(ServerFnError::new(e));
            }
        },
    })
}

/// Get a specific artists track
///
/// # Arguments
/// * `pool` - The database connection pool
/// * `user` - The current user
/// * `artist_slug` - The slug of the artist
/// * `release_slug` - The slug of the release
/// * `track_slug` - The slug of the track
///
/// # Returns
/// The track for the artist
///
/// # Errors
/// If the artist cannot be found, return an error
/// If the release cannot be found, return an error
/// If the user cannot be found, return an error
/// If the track cannot be found, return an error
#[cfg(feature = "ssr")]
pub async fn get_track_service(
    pool: &PgPool,
    user: Option<&User>,
    artist_slug: String,
    release_slug: String,
    track_slug: String,
) -> Result<TrackResult, ServerFnError> {
    let artist = match Artist::get_by_slug(pool, artist_slug).await {
        Ok(artist) => artist,
        Err(e) => {
            let err = format!("Error while getting artist: {e:?}");
            tracing::error!("{err}");
            return Err(ServerFnError::new(e));
        }
    };
    let release = match Release::get_by_slug(pool, release_slug).await {
        Ok(release) => release,
        Err(e) => {
            let err = format!("Error while getting release: {e:?}");
            tracing::error!("{err}");
            return Err(ServerFnError::new(e));
        }
    };

    let Some(current_user) = user else {
        return Err(ServerFnError::new("User not found"));
    };
    let include_hidden = current_user.permissions.contains("label_owner");

    let track = Track::get_by_release_and_artist_and_record_label_and_slug(
        pool,
        release.id,
        artist.id,
        artist.label_id,
        track_slug.clone(),
        include_hidden,
    )
    .await
    .map_err(|e| {
        let err = format!("Error while getting track: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;

    let artists = track.get_artists(pool).await.map_err(|e| {
        let err = format!("Error while getting artists: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;

    let releases = track.get_releases(pool).await.map_err(|e| {
        let err = format!("Error while getting releases: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;

    Ok(TrackResult {
        track,
        artists,
        releases,
    })
}

/// Create a new track
///
/// # Arguments
/// pool: `PgPool` - The database connection pool
/// user: Option<&User> - The user creating the track
/// `form`: `CreateTrackForm` - The form to create the track
///
/// # Returns
/// Result<`TrackResult`, `ServerFnError`> - The created track
///
/// # Errors
/// If the name is empty, return an error
/// If the track cannot be created, return an error
/// If the user does not have the required permissions, return an error
#[cfg(feature = "ssr")]
pub async fn create_track_service(
    pool: &PgPool,
    user: Option<&User>,
    form: CreateTrackForm,
) -> Result<TrackResult, ServerFnError> {
    match user_with_permissions(user, vec!["admin", "label_owner"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }

    let track = Track::create(
        pool,
        form.name,
        form.description,
        form.primary_artist_id,
        form.isrc_code,
        form.bpm,
        form.published_at,
    )
    .await
    .map_err(|e| {
        let err = format!("Error while creating track: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;

    let artist_ids = form
        .artist_ids
        .split(',')
        .filter_map(|s| s.parse::<i64>().ok())
        .collect::<Vec<i64>>();
    track.set_artists(pool, artist_ids).await.map_err(|e| {
        let err = format!("Error while setting artists: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;
    let artists = track.get_artists(pool).await.map_err(|e| {
        let err = format!("Error while getting artists: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;

    let release_ids = form
        .release_ids
        .split(',')
        .filter_map(|s| s.parse::<i64>().ok())
        .collect::<Vec<i64>>();
    track.set_releases(pool, release_ids).await.map_err(|e| {
        let err = format!("Error while setting releases: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;
    let releases = track.get_releases(pool).await.map_err(|e| {
        let err = format!("Error while getting releases: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;

    Ok(TrackResult {
        track,
        artists,
        releases,
    })
}

/// Update a track
///
/// # Arguments
/// `pool`: `PgPool` - The database connection pool
/// `user`: `Option<&User>` - The user creating the track
/// `form`: `CreateTrackForm` - The form to create the track
///
/// # Returns
/// Result<`TrackResult`, `ServerFnError`> - The created track
///
/// # Errors
/// If the name is empty, return an error
/// If the track cannot be created, return an error
/// If the user does not have the required permissions, return an error
#[cfg(feature = "ssr")]
pub async fn update_track_service(
    pool: &PgPool,
    user: Option<&User>,
    form: UpdateTrackForm,
) -> Result<TrackResult, ServerFnError> {
    match user_with_permissions(user, vec!["admin", "label_owner"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }

    let mut track = Track::get_by_slug(pool, form.slug).await.map_err(|e| {
        let err = format!("Error while getting track by slug: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;

    track.name = form.name;
    track.description = form.description;
    track.primary_artist_id = form.primary_artist_id;
    track.isrc_code = form.isrc_code;
    track.bpm = form.bpm;
    track.published_at = form.published_at;

    track = track.update(pool).await.map_err(|e| {
        let err = format!("Error while updating track: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;

    let artist_ids = form
        .artist_ids
        .split(',')
        .filter_map(|s| s.parse::<i64>().ok())
        .collect::<Vec<i64>>();
    track.set_artists(pool, artist_ids).await.map_err(|e| {
        let err = format!("Error while setting artists: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;
    let artists = track.get_artists(pool).await.map_err(|e| {
        let err = format!("Error while getting artists: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;
    let release_ids = form
        .release_ids
        .split(',')
        .filter_map(|s| s.parse::<i64>().ok())
        .collect::<Vec<i64>>();
    track.set_releases(pool, release_ids).await.map_err(|e| {
        let err = format!("Error while setting releases: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;
    let releases = track.get_releases(pool).await.map_err(|e| {
        let err = format!("Error while getting releases: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;

    Ok(TrackResult {
        track,
        artists,
        releases,
    })
}

/// Soft delete a track
///
/// # Arguments
/// pool: `PgPool` - The database connection pool
/// user: Option<&User> - The user deleting the track
/// slug: String - The slug of the track
///
/// # Returns
/// Result<`TrackResult`, `ServerFnError`> - The deleted track
///
/// # Errors
/// If the track cannot be found, return an error
/// If the user does not have the required permissions, return an error
#[cfg(feature = "ssr")]
pub async fn delete_track_service(
    pool: &PgPool,
    user: Option<&User>,
    slug: String,
) -> Result<TrackResult, ServerFnError> {
    match user_with_permissions(user, vec!["admin", "label_owner"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }

    let track = Track::get_by_slug(pool, slug).await.map_err(|e| {
        let err = format!("Error while getting track: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;

    Ok(TrackResult {
        track: track.delete(pool).await.map_err(|e| {
            let err = format!("Error while deleting track: {e:?}");
            tracing::error!("{err}");
            ServerFnError::new(e)
        })?,
        artists: track.get_artists(pool).await.map_err(|e| {
            let err = format!("Error while getting artists: {e:?}");
            tracing::error!("{err}");
            ServerFnError::new(e)
        })?,
        releases: track.get_releases(pool).await.map_err(|e| {
            let err = format!("Error while getting releases: {e:?}");
            tracing::error!("{err}");
            ServerFnError::new(e)
        })?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "ssr")]
    use crate::models::test_helpers::{
        create_test_artist, create_test_record_label, create_test_release, create_test_track,
        create_test_user, create_test_user_with_permissions,
    };

    #[sqlx::test]
    async fn test_get_tracks_service_admin_user(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user: User = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();
        let track = create_test_track(&pool, 1, Some(release.clone()), Some(artist.clone()))
            .await
            .unwrap();
        let mut unpublished_track =
            create_test_track(&pool, 2, Some(release.clone()), Some(artist.clone()))
                .await
                .unwrap();
        unpublished_track.published_at = None;
        unpublished_track.clone().update(&pool).await.unwrap();

        let tracks = get_tracks_service(
            &pool,
            Some(&user),
            artist.slug.clone(),
            release.slug.clone(),
        )
        .await
        .unwrap();
        assert_eq!(tracks.tracks.len(), 2);
        assert_eq!(tracks.tracks[0].id, track.id);
        assert_eq!(tracks.tracks[1].id, unpublished_track.id);
    }

    #[sqlx::test]
    async fn test_get_tracks_service_no_user(pool: PgPool) {
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();
        let track = create_test_track(&pool, 1, Some(release.clone()), Some(artist.clone()))
            .await
            .unwrap();
        let mut unpublished_track =
            create_test_track(&pool, 2, Some(release.clone()), Some(artist.clone()))
                .await
                .unwrap();
        unpublished_track.published_at = None;
        unpublished_track.clone().update(&pool).await.unwrap();

        let tracks = get_tracks_service(&pool, None, artist.slug.clone(), release.slug.clone())
            .await
            .unwrap();

        assert_eq!(tracks.tracks.len(), 1);
        assert_eq!(tracks.tracks[0].id, track.id);
    }

    #[sqlx::test]
    async fn test_get_tracks_service_anonymous_user(pool: PgPool) {
        let (user, _) = create_test_user(&pool, 1).await.unwrap().into_user(None);

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();
        let track = create_test_track(&pool, 1, Some(release.clone()), Some(artist.clone()))
            .await
            .unwrap();
        let mut unpublished_track =
            create_test_track(&pool, 2, Some(release.clone()), Some(artist.clone()))
                .await
                .unwrap();
        unpublished_track.published_at = None;
        unpublished_track.clone().update(&pool).await.unwrap();

        let tracks = get_tracks_service(
            &pool,
            Some(&user),
            artist.slug.clone(),
            release.slug.clone(),
        )
        .await
        .unwrap();

        assert_eq!(tracks.tracks.len(), 1);
        assert_eq!(tracks.tracks[0].id, track.id);
    }

    #[sqlx::test]
    async fn test_get_track_service(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();
        let track = create_test_track(&pool, 1, Some(release.clone()), Some(artist.clone()))
            .await
            .unwrap();

        let track_result = get_track_service(
            &pool,
            Some(&user),
            artist.slug.clone(),
            release.slug.clone(),
            track.slug.clone(),
        )
        .await
        .unwrap();

        assert_eq!(track_result.track.id, track.id);
    }

    #[sqlx::test]
    async fn test_get_track_service_no_permission(pool: PgPool) {
        let permissions = vec![];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();
        let mut unpublished_track =
            create_test_track(&pool, 1, Some(release.clone()), Some(artist.clone()))
                .await
                .unwrap();
        unpublished_track.published_at = None;
        unpublished_track.clone().update(&pool).await.unwrap();

        let track_result = get_track_service(
            &pool,
            Some(&user),
            artist.slug.clone(),
            release.slug.clone(),
            unpublished_track.slug.clone(),
        )
        .await;
        assert!(track_result.is_err());
        assert_eq!(
            track_result.unwrap_err().to_string(),
            "error running server function: Could not find track test-track-1 for release with id 1 and artist with id 1 and record label with id 1."
                .to_string()
        );
    }

    #[sqlx::test]
    async fn test_create_track_service(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user: User = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();

        let form = CreateTrackForm {
            name: "Test Track".to_string(),
            description: "Test Track Description".to_string(),
            primary_artist_id: artist.id,
            isrc_code: Some("UKUTX2020123".to_string()),
            bpm: Some(120),
            published_at: Some(chrono::Utc::now()),
            artist_ids: artist.id.to_string(),
            release_ids: release.id.to_string(),
        };

        let track_result = create_track_service(&pool, Some(&user), form.clone())
            .await
            .unwrap();
        assert_eq!(track_result.track.name, "Test Track");
        assert_eq!(track_result.track.description, "Test Track Description");
        assert_eq!(track_result.track.primary_artist_id, artist.id);
        assert_eq!(
            track_result.track.isrc_code,
            Some("UKUTX2020123".to_string())
        );
        assert_eq!(track_result.track.bpm, Some(120));
        assert!(track_result.track.published_at.is_some());
        assert_eq!(track_result.artists.len(), 1);
        assert_eq!(track_result.artists[0].id, artist.id);
        assert_eq!(track_result.releases.len(), 1);
        assert_eq!(track_result.releases[0].id, release.id);
    }

    #[sqlx::test]
    async fn test_create_track_service_no_permision(pool: PgPool) {
        let permissions = vec![];
        let user: User = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();

        let form = CreateTrackForm {
            name: "Test Track".to_string(),
            description: "Test Track Description".to_string(),
            primary_artist_id: artist.id,
            isrc_code: Some("UKUTX2020123".to_string()),
            bpm: Some(120),
            published_at: Some(chrono::Utc::now()),
            artist_ids: artist.id.to_string(),
            release_ids: release.id.to_string(),
        };

        let track_result = create_track_service(&pool, Some(&user), form).await;
        assert!(track_result.is_err());
        assert_eq!(
            track_result.unwrap_err().to_string(),
            "error running server function: You do not have permission.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_update_track_service_no_permission(pool: PgPool) {
        let permissions = vec![];
        let user: User = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();

        let form = CreateTrackForm {
            name: "Test Track".to_string(),
            description: "Test Track Description".to_string(),
            primary_artist_id: artist.id,
            isrc_code: Some("UKUTX2020123".to_string()),
            bpm: Some(120),
            published_at: Some(chrono::Utc::now()),
            artist_ids: artist.id.to_string(),
            release_ids: release.id.to_string(),
        };

        let track_result = create_track_service(&pool, Some(&user), form).await;
        assert!(track_result.is_err());
        assert_eq!(
            track_result.unwrap_err().to_string(),
            "error running server function: You do not have permission.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_update_track_service(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user: User = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();
        let release2 = create_test_release(&pool, 2, Some(artist.clone()))
            .await
            .unwrap();

        let form = CreateTrackForm {
            name: "Test Track".to_string(),
            description: "Test Track Description".to_string(),
            isrc_code: Some("UKUTX2020123".to_string()),
            bpm: Some(120),
            primary_artist_id: artist.id,
            published_at: Some(chrono::Utc::now()),
            artist_ids: artist.id.to_string(),
            release_ids: release.id.to_string(),
        };

        let track_result = create_track_service(&pool, Some(&user), form).await;
        println!("Update Result: {track_result:?}");

        assert!(track_result.is_ok());
        let track = track_result.unwrap();

        let update_form = UpdateTrackForm {
            name: "Updated Track".to_string(),
            slug: "test-track".to_string(),
            description: "Updated Track Description".to_string(),
            primary_artist_id: artist.id,
            isrc_code: Some("UKUTX2025321".to_string()),
            bpm: Some(130),
            published_at: Some(chrono::Utc::now()),
            artist_ids: artist.id.to_string(),
            release_ids: [release.id.to_string(), release2.id.to_string()].join(","),
        };

        let update_result = update_track_service(&pool, Some(&user), update_form.clone()).await;
        assert!(update_result.is_ok());
        let updated_track = update_result.unwrap();

        assert_eq!(track.track.id, updated_track.track.id);
        assert_eq!(updated_track.track.name, "Updated Track");
        assert_eq!(updated_track.track.description, "Updated Track Description");
        assert_eq!(updated_track.track.primary_artist_id, artist.id);
        assert_eq!(
            updated_track.track.isrc_code,
            Some("UKUTX2025321".to_string())
        );
        assert_eq!(updated_track.track.bpm, Some(130));
        assert!(updated_track.track.published_at.is_some());
        assert_eq!(updated_track.artists, vec![artist]);
        assert_eq!(updated_track.releases, vec![release, release2]);
    }

    #[sqlx::test]
    pub fn delete_track(pool: sqlx::PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user: User = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();

        let form = CreateTrackForm {
            name: "Test Track".to_string(),
            description: "Test Track Description".to_string(),
            primary_artist_id: artist.id,
            isrc_code: Some("UKUTX2020123".to_string()),
            bpm: Some(120),
            published_at: Some(chrono::Utc::now()),
            artist_ids: artist.id.to_string(),
            release_ids: release.id.to_string(),
        };

        let track_result = create_track_service(&pool, Some(&user), form).await;
        assert!(track_result.is_ok());
        let track = track_result.unwrap();

        let delete_result =
            delete_track_service(&pool, Some(&user), track.track.slug.clone()).await;
        assert!(delete_result.is_ok());

        let get_result = get_track_service(
            &pool,
            Some(&user),
            artist.slug,
            release.slug,
            track.track.slug,
        )
        .await;
        assert!(get_result.is_ok());
        assert!(get_result.unwrap().track.deleted_at.is_some());
    }

    #[sqlx::test]
    pub fn delete_track_not_found(pool: sqlx::PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let delete_result = delete_track_service(&pool, Some(&user), "not-found".to_string()).await;
        assert!(delete_result.is_err());
        assert_eq!(
            delete_result.unwrap_err().to_string(),
            "error running server function: Could not find track with slug not-found.".to_string()
        );
    }

    #[sqlx::test]
    pub fn delete_service_no_permission(pool: sqlx::PgPool) {
        let permissions = vec![];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();
        let track = create_test_track(&pool, 1, Some(release.clone()), Some(artist.clone()))
            .await
            .unwrap();

        let delete_result = delete_track_service(&pool, Some(&user), track.slug.clone()).await;
        assert_eq!(
            delete_result.unwrap_err().to_string(),
            "error running server function: You do not have permission.".to_string()
        );

        let get_result =
            get_track_service(&pool, Some(&user), artist.slug, release.slug, track.slug).await;
        assert!(get_result.is_ok());
    }
}
