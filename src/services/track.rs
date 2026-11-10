//! Services for the tracks routes
use crate::api_types::track::{ContinuationResult, TrackResult, UpcomingAudio};
use crate::error::AppError;
use sqlx::PgPool;

use super::authentication_helpers::user_with_permissions;
use crate::forms::track::{CreateTrackForm, UpdateTrackForm};
use crate::models::{
    artist::Artist, auth::User, release::Release, track::Track,
    track_with_artists::TrackWithArtists,
};

/// `Artist::find_by_slug` wired onto the service error ladder: a missing
/// row is a 404 with the model's message, a database failure stays a 500.
#[cfg(feature = "server")]
async fn lookup_artist(pool: &PgPool, slug: String) -> Result<Artist, AppError> {
    Artist::find_by_slug(pool, slug.clone())
        .await
        .map_err(|e| {
            let err = format!("Error while getting artist: {e:?}");
            tracing::error!("{err}");
            AppError::internal(e)
        })?
        .ok_or_else(|| AppError::NotFound(format!("Could not find artist with slug {slug}.")))
}

/// `Release::find_by_slug` wired onto the service error ladder, as above.
#[cfg(feature = "server")]
async fn lookup_release(pool: &PgPool, slug: String) -> Result<Release, AppError> {
    Release::find_by_slug(pool, slug.clone())
        .await
        .map_err(|e| {
            let err = format!("Error while getting release: {e:?}");
            tracing::error!("{err}");
            AppError::internal(e)
        })?
        .ok_or_else(|| AppError::NotFound(format!("Could not find release with slug {slug}.")))
}

/// `Track::find_by_slug` wired onto the service error ladder, as above.
#[cfg(feature = "server")]
async fn find_track_by_slug(pool: &PgPool, slug: &str) -> Result<Track, AppError> {
    Track::find_by_slug(pool, slug.to_string())
        .await
        .map_err(|e| {
            let err = format!("Error while getting track: {e:?}");
            tracing::error!("{err}");
            AppError::internal(e)
        })?
        .ok_or_else(|| AppError::NotFound(format!("Could not find track with slug {slug}.")))
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
#[cfg(feature = "server")]
pub async fn get_track_service(
    pool: &PgPool,
    user: Option<&User>,
    artist_slug: String,
    release_slug: String,
    track_slug: String,
) -> Result<TrackResult, AppError> {
    let artist = lookup_artist(pool, artist_slug).await?;
    let release = lookup_release(pool, release_slug).await?;

    let Some(current_user) = user else {
        return Err(AppError::Unauthorized("User not found".into()));
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
        AppError::internal(e)
    })?;

    let artists = track.get_artists(pool).await.map_err(|e| {
        let err = format!("Error while getting artists: {e:?}");
        tracing::error!("{err}");
        AppError::internal(e)
    })?;

    Ok(TrackResult { track, artists })
}

/// How many tracks one continuation batch offers the global player.
///
/// Small on purpose: the listener reaches the end again soon and gets a fresh
/// random pick rather than a whole discography dumped into the queue.
#[cfg(feature = "server")]
pub const CONTINUATION_BATCH: i64 = 5;

/// Tracks to top the global player's queue up with when it runs out.
///
/// Up to [`CONTINUATION_BATCH`] audio tracks led by `artist_slug`, picked at
/// random and skipping the slugs in `exclude_slugs` (what the queue already
/// holds). Visibility is the public one: only published, undeleted tracks on
/// published, undeleted releases answer, whoever the session is.
///
/// # Errors
/// 404 when the slug matches no artist, 500 on database failure.
#[cfg(feature = "server")]
pub async fn continuation_tracks_service(
    pool: &PgPool,
    artist_slug: String,
    exclude_slugs: Vec<String>,
) -> Result<ContinuationResult, AppError> {
    let artist = lookup_artist(pool, artist_slug).await?;

    let tracks =
        Track::list_playable_by_artist_id(pool, artist.id, &exclude_slugs, CONTINUATION_BATCH)
            .await
            .map_err(|e| {
                let err = format!("Error while getting continuation tracks: {e:?}");
                tracing::error!("{err}");
                AppError::internal(e)
            })?;

    let mut upcoming = Vec::with_capacity(tracks.len());
    for track in tracks {
        // Same shape the release page's rows carry, so the player maps them
        // the same way; the batch is at most `CONTINUATION_BATCH` long, so
        // this per-track loading matches `Release::get_tracks`' own pattern.
        let artists = track.get_artists(pool).await.map_err(|e| {
            let err = format!("Error while getting track artists: {e:?}");
            tracing::error!("{err}");
            AppError::internal(e)
        })?;
        let release = Release::get_by_id(pool, track.release_id)
            .await
            .map_err(|e| {
                let err = format!("Error while getting the track's release: {e:?}");
                tracing::error!("{err}");
                AppError::internal(e)
            })?;
        upcoming.push(UpcomingAudio {
            track: TrackWithArtists { track, artists },
            release_name: release.name,
        });
    }

    Ok(ContinuationResult { tracks: upcoming })
}

/// Create a new track
///
/// # Arguments
/// pool: `PgPool` - The database connection pool
/// user: Option<&User> - The user creating the track
/// `form`: `CreateTrackForm` - The form to create the track
///
/// # Returns
/// Result<`TrackResult`, `AppError`> - The created track
///
/// # Errors
/// If the name is empty, return an error
/// If the track cannot be created, return an error
/// If the user does not have the required permissions, return an error
#[cfg(feature = "server")]
pub async fn create_track_service(
    pool: &PgPool,
    user: Option<&User>,
    form: CreateTrackForm,
) -> Result<TrackResult, AppError> {
    match user_with_permissions(user, vec!["admin", "label_owner"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }

    let track = Track::create(
        pool,
        form.name,
        form.description,
        form.lyrics,
        form.primary_artist_id,
        form.release_id,
        form.isrc_code,
        form.bpm,
        form.track_number,
        form.published_at,
    )
    .await
    .map_err(|e| {
        let err = format!("Error while creating track: {e:?}");
        tracing::error!("{err}");
        AppError::internal(e)
    })?;

    let artist_ids = form
        .artist_ids
        .split(',')
        .filter_map(|s| s.parse::<i64>().ok())
        .collect::<Vec<i64>>();
    track.set_artists(pool, artist_ids).await.map_err(|e| {
        let err = format!("Error while setting artists: {e:?}");
        tracing::error!("{err}");
        AppError::internal(e)
    })?;
    let artists = track.get_artists(pool).await.map_err(|e| {
        let err = format!("Error while getting artists: {e:?}");
        tracing::error!("{err}");
        AppError::internal(e)
    })?;

    Ok(TrackResult { track, artists })
}

/// Update a track
///
/// # Arguments
/// `pool`: `PgPool` - The database connection pool
/// `user`: `Option<&User>` - The user creating the track
/// `form`: `CreateTrackForm` - The form to create the track
///
/// # Returns
/// Result<`TrackResult`, `AppError`> - The created track
///
/// # Errors
/// If the name is empty, return an error
/// If the track cannot be created, return an error
/// If the user does not have the required permissions, return an error
#[cfg(feature = "server")]
pub async fn update_track_service(
    pool: &PgPool,
    user: Option<&User>,
    form: UpdateTrackForm,
) -> Result<TrackResult, AppError> {
    match user_with_permissions(user, vec!["admin", "label_owner"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }

    let mut track = find_track_by_slug(pool, &form.slug).await?;

    track.name = form.name;
    track.description = form.description;
    track.lyrics = form.lyrics;
    track.primary_artist_id = form.primary_artist_id;
    track.release_id = form.release_id;
    track.isrc_code = form.isrc_code;
    track.bpm = form.bpm;
    track.published_at = form.published_at;
    track.track_number = form.track_number;

    track = track.update(pool).await.map_err(|e| {
        let err = format!("Error while updating track: {e:?}");
        tracing::error!("{err}");
        AppError::internal(e)
    })?;

    let artist_ids = form
        .artist_ids
        .split(',')
        .filter_map(|s| s.parse::<i64>().ok())
        .collect::<Vec<i64>>();
    track.set_artists(pool, artist_ids).await.map_err(|e| {
        let err = format!("Error while setting artists: {e:?}");
        tracing::error!("{err}");
        AppError::internal(e)
    })?;
    let artists = track.get_artists(pool).await.map_err(|e| {
        let err = format!("Error while getting artists: {e:?}");
        tracing::error!("{err}");
        AppError::internal(e)
    })?;

    Ok(TrackResult { track, artists })
}

/// Soft delete a track
///
/// # Arguments
/// pool: `PgPool` - The database connection pool
/// user: Option<&User> - The user deleting the track
/// slug: String - The slug of the track
///
/// # Returns
/// Result<`TrackResult`, `AppError`> - The deleted track
///
/// # Errors
/// If the track cannot be found, return an error
/// If the user does not have the required permissions, return an error
#[cfg(feature = "server")]
pub async fn delete_track_service(
    pool: &PgPool,
    user: Option<&User>,
    slug: String,
) -> Result<TrackResult, AppError> {
    match user_with_permissions(user, vec!["admin", "label_owner"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }

    let track = find_track_by_slug(pool, &slug).await?;

    Ok(TrackResult {
        track: track.delete(pool).await.map_err(|e| {
            let err = format!("Error while deleting track: {e:?}");
            tracing::error!("{err}");
            AppError::internal(e)
        })?,
        artists: track.get_artists(pool).await.map_err(|e| {
            let err = format!("Error while getting artists: {e:?}");
            tracing::error!("{err}");
            AppError::internal(e)
        })?,
    })
}

/// Restore a soft deleted track
///
/// # Arguments
/// pool: `PgPool` - The database connection pool
/// user: Option<&User> - The user deleting the track
/// slug: String - The slug of the track
///
/// # Returns
/// Result<`TrackResult`, `AppError`> - The restored track
///
/// # Errors
/// If the track cannot be found, return an error
/// If the user does not have the required permissions, return an error
#[cfg(feature = "server")]
pub async fn restore_track_service(
    pool: &PgPool,
    user: Option<&User>,
    slug: String,
) -> Result<TrackResult, AppError> {
    match user_with_permissions(user, vec!["admin", "label_owner"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }

    let mut track = find_track_by_slug(pool, &slug).await?;
    track.deleted_at = None;
    track.clone().update(pool).await.map_err(|e| {
        let err = format!("Error while restoring track: {e:?}");
        tracing::error!("{err}");
        AppError::internal(e)
    })?;

    Ok(TrackResult {
        track: track.clone(),
        artists: track.get_artists(pool).await.map_err(|e| {
            let err = format!("Error while getting artists: {e:?}");
            tracing::error!("{err}");
            AppError::internal(e)
        })?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "server")]
    use crate::models::test_helpers::{
        create_test_artist, create_test_record_label, create_test_release, create_test_track,
        create_test_user_with_permissions,
    };

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
            "Could not find track test-track-1 for release with id 1 and artist with id 1 and record label with id 1."
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
            lyrics: "Test lyrics".to_string(),
            primary_artist_id: artist.id,
            release_id: release.id,
            isrc_code: Some("UKXXX2020123".to_string()),
            bpm: Some(120),
            published_at: Some(chrono::Utc::now()),
            artist_ids: artist.id.to_string(),
            track_number: 1,
        };

        let track_result = create_track_service(&pool, Some(&user), form.clone())
            .await
            .unwrap();
        assert_eq!(track_result.track.name, "Test Track");
        assert_eq!(track_result.track.description, "Test Track Description");
        assert_eq!(track_result.track.primary_artist_id, artist.id);
        assert_eq!(track_result.track.release_id, release.id);
        assert_eq!(
            track_result.track.isrc_code,
            Some("UKXXX2020123".to_string())
        );
        assert_eq!(track_result.track.bpm, Some(120));
        assert_eq!(track_result.track.track_number, 1);
        assert!(track_result.track.published_at.is_some());
        assert_eq!(track_result.artists.len(), 1);
        assert_eq!(track_result.artists[0].id, artist.id);
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
            lyrics: "Test lyrics".to_string(),
            primary_artist_id: artist.id,
            release_id: release.id,
            isrc_code: Some("UKXXX2020123".to_string()),
            bpm: Some(120),
            published_at: Some(chrono::Utc::now()),
            artist_ids: artist.id.to_string(),
            track_number: 1,
        };

        let track_result = create_track_service(&pool, Some(&user), form).await;
        assert!(track_result.is_err());
        assert_eq!(
            track_result.unwrap_err().to_string(),
            "You do not have permission.".to_string()
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
            lyrics: "Test lyrics".to_string(),
            primary_artist_id: artist.id,
            release_id: release.id,
            isrc_code: Some("UKXX2020123".to_string()),
            bpm: Some(120),
            published_at: Some(chrono::Utc::now()),
            artist_ids: artist.id.to_string(),
            track_number: 1,
        };

        let track_result = create_track_service(&pool, Some(&user), form).await;
        assert!(track_result.is_err());
        assert_eq!(
            track_result.unwrap_err().to_string(),
            "You do not have permission.".to_string()
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
            lyrics: "Test lyrics".to_string(),
            isrc_code: Some("UKXXX2020123".to_string()),
            bpm: Some(120),
            primary_artist_id: artist.id,
            release_id: release.id,
            published_at: Some(chrono::Utc::now()),
            artist_ids: artist.id.to_string(),
            track_number: 1,
        };

        let track_result = create_track_service(&pool, Some(&user), form).await;

        assert!(track_result.is_ok());
        let track = track_result.unwrap();

        let update_form = UpdateTrackForm {
            name: "Updated Track".to_string(),
            slug: "test-track".to_string(),
            description: "Updated Track Description".to_string(),
            lyrics: "Updated lyrics".to_string(),
            primary_artist_id: artist.id,
            release_id: release2.id,
            isrc_code: Some("UKXXX2025321".to_string()),
            bpm: Some(130),
            track_number: 2,
            published_at: Some(chrono::Utc::now()),
            artist_ids: artist.id.to_string(),
        };

        let update_result = update_track_service(&pool, Some(&user), update_form.clone()).await;
        assert!(update_result.is_ok());
        let updated_track = update_result.unwrap();

        assert_eq!(track.track.id, updated_track.track.id);
        assert_eq!(updated_track.track.name, "Updated Track");
        assert_eq!(updated_track.track.description, "Updated Track Description");
        assert_eq!(updated_track.track.lyrics, "Updated lyrics");
        assert_eq!(updated_track.track.primary_artist_id, artist.id);
        assert_eq!(updated_track.track.release_id, release2.id);
        assert_eq!(
            updated_track.track.isrc_code,
            Some("UKXXX2025321".to_string())
        );
        assert_eq!(updated_track.track.bpm, Some(130));
        assert_eq!(updated_track.track.track_number, 2);
        assert!(updated_track.track.published_at.is_some());
        assert_eq!(updated_track.artists, vec![artist]);
    }

    /// An uploaded audio file has to survive the next save of the track it
    /// belongs to: the service loads the whole row and writes every column
    /// back, so the audio column rides along with the edit.
    #[sqlx::test]
    async fn update_track_service_keeps_the_stored_audio(pool: PgPool) {
        let user = create_test_user_with_permissions(&pool, 2, vec!["admin", "label_owner"])
            .await
            .unwrap();
        let artist = create_test_artist(&pool, 2, None).await.unwrap();
        let release = create_test_release(&pool, 2, Some(artist.clone()))
            .await
            .unwrap();
        let track = create_test_track(&pool, 2, Some(release), Some(artist.clone()))
            .await
            .unwrap();
        sqlx::query("UPDATE tracks SET audio = $1 WHERE id = $2")
            .bind("1700000000-castles.flac")
            .bind(track.id)
            .execute(&pool)
            .await
            .expect("the audio column to be set");

        let form = UpdateTrackForm {
            name: "Renamed Track".to_string(),
            slug: track.slug.clone(),
            description: track.description.clone(),
            lyrics: track.lyrics.clone(),
            primary_artist_id: artist.id,
            release_id: track.release_id,
            isrc_code: track.isrc_code.clone(),
            bpm: track.bpm,
            track_number: track.track_number,
            published_at: track.published_at,
            artist_ids: artist.id.to_string(),
        };
        let saved = update_track_service(&pool, Some(&user), form)
            .await
            .expect("the save");

        assert_eq!(saved.track.name, "Renamed Track");
        assert_eq!(
            saved.track.audio.as_deref(),
            Some("1700000000-castles.flac"),
            "editing a track must not strand its audio file"
        );
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
            lyrics: "Test lyrics".to_string(),
            primary_artist_id: artist.id,
            release_id: release.id,
            isrc_code: Some("UKXXX2020123".to_string()),
            bpm: Some(120),
            track_number: 1,
            published_at: Some(chrono::Utc::now()),
            artist_ids: artist.id.to_string(),
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
            "Could not find track with slug not-found.".to_string()
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
            "You do not have permission.".to_string()
        );

        let get_result =
            get_track_service(&pool, Some(&user), artist.slug, release.slug, track.slug).await;
        assert!(get_result.is_ok());
    }

    #[sqlx::test]
    async fn test_restore_track_service(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let track = create_test_track(&pool, 1, None, None).await.unwrap();
        let track = track.delete(&pool).await.unwrap();
        let restored_track = restore_track_service(&pool, Some(&user), track.slug.clone())
            .await
            .unwrap();

        assert!(restored_track.track.deleted_at.is_none());
    }

    #[sqlx::test]
    async fn test_restore_track_service_no_track(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let restored_track = restore_track_service(&pool, Some(&user), "missing".to_string()).await;
        assert!(restored_track.is_err());
        assert_eq!(
            restored_track.unwrap_err().to_string(),
            "Could not find track with slug missing.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_restore_track_service_no_user(pool: PgPool) {
        let track = create_test_track(&pool, 1, None, None).await.unwrap();
        let track = track.delete(&pool).await.unwrap();
        let restored_track =
            restore_track_service(&pool, Some(&User::default()), track.slug.clone()).await;
        assert!(restored_track.is_err());
        assert_eq!(
            restored_track.unwrap_err().to_string(),
            "You must be logged in to view this page.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_restore_track_service_no_permissions(pool: PgPool) {
        let user = create_test_user_with_permissions(&pool, 1, vec![])
            .await
            .unwrap();
        let track = create_test_track(&pool, 1, None, None).await.unwrap();
        let track = track.delete(&pool).await.unwrap();
        let restored_track = restore_track_service(&pool, Some(&user), track.slug.clone()).await;
        assert!(restored_track.is_err());
        assert_eq!(
            restored_track.unwrap_err().to_string(),
            "You do not have permission.".to_string()
        );
    }

    /// Give a track an audio file name. Uploads own the `audio` column in the
    /// model layer (an update deliberately keeps a stored file), so these
    /// fixtures set it straight, as the SSR tests in `crate::server` do.
    async fn give_audio(pool: &PgPool, track: &Track, file: &str) {
        sqlx::query("UPDATE tracks SET audio = $1 WHERE id = $2")
            .bind(file)
            .bind(track.id)
            .execute(pool)
            .await
            .expect("stored audio");
    }

    /// The slugs the service offered, sorted: the batch arrives in random
    /// order, so the pins assert membership rather than sequence.
    fn continuation_slugs(result: &ContinuationResult) -> Vec<String> {
        let mut slugs: Vec<String> = result
            .tracks
            .iter()
            .map(|upcoming| upcoming.track.track.slug.clone())
            .collect();
        slugs.sort();
        slugs
    }

    #[sqlx::test]
    async fn continuation_offers_the_artist_audio_tracks(pool: PgPool) {
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label))
            .await
            .unwrap();
        let release_one = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();
        let release_two = create_test_release(&pool, 2, Some(artist.clone()))
            .await
            .unwrap();
        let on_one = create_test_track(&pool, 1, Some(release_one), Some(artist.clone()))
            .await
            .unwrap();
        let on_two = create_test_track(&pool, 2, Some(release_two.clone()), Some(artist.clone()))
            .await
            .unwrap();
        let _silent = create_test_track(&pool, 3, Some(release_two), Some(artist.clone()))
            .await
            .unwrap();
        give_audio(&pool, &on_one, "1700000000-one.wav").await;
        give_audio(&pool, &on_two, "1700000000-two.wav").await;

        let result = continuation_tracks_service(&pool, artist.slug.clone(), Vec::new())
            .await
            .unwrap();

        assert_eq!(
            continuation_slugs(&result),
            vec!["test-track-1", "test-track-2"],
            "tracks with audio, on either release, and no silent row"
        );
        // The bar shows where each fetched track came from, and its featured
        // artists come along in the same shape the release page carries.
        let by_slug: std::collections::BTreeMap<_, _> = result
            .tracks
            .iter()
            .map(|upcoming| (upcoming.track.track.slug.as_str(), upcoming))
            .collect();
        assert_eq!(by_slug["test-track-1"].release_name, "Test Release 1");
        assert_eq!(by_slug["test-track-2"].release_name, "Test Release 2");
        assert_eq!(
            by_slug["test-track-1"].track.artists.len(),
            1,
            "the featured artists come along in the same shape the release page carries"
        );
        assert!(
            result
                .tracks
                .iter()
                .all(|upcoming| upcoming.track.track.audio_url().is_some()),
            "everything offered is playable"
        );
    }

    #[sqlx::test]
    async fn continuation_leaves_out_the_queued_slugs(pool: PgPool) {
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label))
            .await
            .unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();
        let first = create_test_track(&pool, 1, Some(release.clone()), Some(artist.clone()))
            .await
            .unwrap();
        let second = create_test_track(&pool, 2, Some(release), Some(artist.clone()))
            .await
            .unwrap();
        give_audio(&pool, &first, "1700000000-one.wav").await;
        give_audio(&pool, &second, "1700000000-two.wav").await;

        // The live queue holds `first` (and names it twice, as Add to Queue
        // may): the fetch must offer `second` only.
        let excluded = vec![first.slug.clone(), first.slug.clone()];
        let result = continuation_tracks_service(&pool, artist.slug.clone(), excluded)
            .await
            .unwrap();
        assert_eq!(continuation_slugs(&result), vec!["test-track-2"]);
    }

    #[sqlx::test]
    async fn continuation_shields_the_listener_from_invisible_rows(pool: PgPool) {
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label))
            .await
            .unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();
        // One audio track for each way a row can hide from the public: the
        // track itself unpublished, soft-deleted, or dated ahead, and the
        // track sitting under a soft-deleted or unpublished release.
        let unpublished = create_test_track(&pool, 1, Some(release.clone()), Some(artist.clone()))
            .await
            .unwrap();
        let deleted = create_test_track(&pool, 2, Some(release.clone()), Some(artist.clone()))
            .await
            .unwrap();
        let deleted = deleted.delete(&pool).await.unwrap();
        let future = create_test_track(&pool, 3, Some(release), Some(artist.clone()))
            .await
            .unwrap();
        // `None` releases: each gets its own, created along with the track.
        let on_deleted_release = create_test_track(&pool, 4, None, Some(artist.clone()))
            .await
            .unwrap();
        let on_unpublished_release = create_test_track(&pool, 5, None, Some(artist.clone()))
            .await
            .unwrap();

        // The visibility shifts themselves go straight to SQL: they are the
        // rows' stored state, not model edits.
        sqlx::query("UPDATE tracks SET published_at = NULL WHERE slug = $1")
            .bind("test-track-1")
            .execute(&pool)
            .await
            .expect("unpublished track");
        sqlx::query("UPDATE tracks SET published_at = now() + interval '1 day' WHERE slug = $1")
            .bind("test-track-3")
            .execute(&pool)
            .await
            .expect("future track");
        sqlx::query("UPDATE releases SET deleted_at = now() WHERE id = $1")
            .bind(on_deleted_release.release_id)
            .execute(&pool)
            .await
            .expect("soft-deleted release");
        sqlx::query("UPDATE releases SET published_at = NULL WHERE id = $1")
            .bind(on_unpublished_release.release_id)
            .execute(&pool)
            .await
            .expect("unpublished release");
        // Every one of them names an audio file: it is visibility, not
        // silence, that leaves them out.
        for track in [
            &unpublished,
            &deleted,
            &future,
            &on_deleted_release,
            &on_unpublished_release,
        ] {
            give_audio(&pool, track, "1700000000-hidden.wav").await;
        }

        let result = continuation_tracks_service(&pool, artist.slug.clone(), Vec::new())
            .await
            .unwrap();
        assert!(
            result.tracks.is_empty(),
            "nothing a guest should not already be able to open"
        );
    }

    #[sqlx::test]
    async fn continuation_caps_the_batch(pool: PgPool) {
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label))
            .await
            .unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();
        for n in 1..=7 {
            let track = create_test_track(&pool, n, Some(release.clone()), Some(artist.clone()))
                .await
                .unwrap();
            give_audio(&pool, &track, &format!("1700000000-track-{n}.wav")).await;
        }

        let result = continuation_tracks_service(&pool, artist.slug.clone(), Vec::new())
            .await
            .unwrap();
        assert_eq!(
            result.tracks.len(),
            usize::try_from(CONTINUATION_BATCH).unwrap(),
            "a discography stays out of the queue: the next end asks for more"
        );
    }

    #[sqlx::test]
    async fn continuation_unknown_artist_is_not_found(pool: PgPool) {
        let result = continuation_tracks_service(&pool, "nobody".to_string(), Vec::new()).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Could not find artist with slug nobody.".to_string()
        );
    }
}
