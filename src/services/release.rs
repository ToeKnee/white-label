//! Services for the releases routes
use crate::api_types::release::{ReleaseResult, ReleasesResult};
use crate::error::AppError;
use sqlx::PgPool;

use super::authentication_helpers::user_with_permissions;
use super::visibility_for;
use crate::forms::release::{CreateReleaseForm, UpdateReleaseForm};
use crate::models::{Visibility, artist::Artist, auth::User, release::Release};

/// `Release::find_by_slug` wired onto the service error ladder: a missing
/// row is a 404 with the model's message, a database failure stays a 500.
#[cfg(feature = "server")]
async fn find_release_by_slug(pool: &PgPool, slug: &str) -> Result<Release, AppError> {
    Release::find_by_slug(pool, slug.to_string())
        .await
        .map_err(|e| {
            let err = format!("Error while getting release by slug: {e:?}");
            tracing::error!("{err}");
            AppError::internal(e)
        })?
        .ok_or_else(|| AppError::NotFound(format!("Could not find release with slug {slug}.")))
}

/// Get the next scheduled release, optionally filtered by artist slug.
///
/// # Arguments
/// * `pool` - The database connection pool
/// * `Option<artist_slug>` - The slug of the artist to filter by
///
/// # Returns
/// The next scheduled release, or None if there are no scheduled releases.
///
/// # Errors
/// If there is an error querying the database, return an error
#[cfg(feature = "server")]
pub async fn get_next_scheduled_release_service(
    pool: &PgPool,
    artist_id: Option<i64>,
    label_id: i64,
) -> Result<Option<ReleaseResult>, AppError> {
    let release = match Release::get_next_scheduled_release(pool, artist_id, label_id).await {
        Ok(release) => Ok(release),
        Err(e) => {
            let err = format!("Error while getting next scheduled release: {e:?}");
            tracing::error!("{err}");
            Err(AppError::internal(e))
        }
    };
    match release {
        Ok(Some(release)) => {
            let artists = release.get_artists(pool).await.map_err(|e| {
                let err = format!("Error while getting artists: {e:?}");
                tracing::error!("{err}");
                AppError::internal(e)
            })?;
            // The countdown previews an unreleased (future) release, so its
            // track list is read at `Hidden`: unpublished tracks stay
            // visible for the preview, deleted ones do not belong.
            let tracks = release
                .get_tracks(pool, Visibility::Hidden)
                .await
                .map_err(|e| {
                    let err = format!("Error while getting tracks: {e:?}");
                    tracing::error!("{err}");
                    AppError::internal(e)
                })?;

            Ok(Some(ReleaseResult {
                release,
                artists,
                tracks,
                // The countdown preview never plays, so the guest allowance
                // is left at the default.
                guest_plays: None,
            }))
        }
        Ok(None) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Get an artists releases
///
/// # Arguments
/// * `pool` - The database connection pool
/// * `user` - The current user
/// * `slug` - The slug of the artist
///
/// # Returns
/// The releases for the artist
///
/// # Errors
/// If the artist cannot be found, return an error
/// If the user cannot be found, return an error
/// If the releases cannot be found, return an error
#[cfg(feature = "server")]
pub async fn get_releases_service(
    pool: &PgPool,
    user: Option<&User>,
    slug: String,
    include_deleted: bool,
) -> Result<ReleasesResult, AppError> {
    let artist = match Artist::find_by_slug(pool, slug.clone()).await {
        Ok(Some(artist)) => artist,
        // Missing artist answers 404 with the model's message; database
        // failures stay 500.
        Ok(None) => {
            return Err(AppError::NotFound(format!(
                "Could not find artist with slug {slug}."
            )));
        }
        Err(e) => {
            let err = format!("Error while getting artist: {e:?}");
            tracing::error!("get_releases_service {err}");
            return Err(AppError::internal(e));
        }
    };

    let visibility = visibility_for(user, include_deleted);

    Ok(ReleasesResult {
        releases: match Release::list_by_artist_and_record_label(
            pool,
            artist.id,
            artist.label_id,
            visibility,
        )
        .await
        {
            Ok(releases) => releases,
            Err(e) => {
                let err = format!("Error while getting releases: {e:?}");
                tracing::error!("{err}");
                return Err(AppError::internal(e));
            }
        },
    })
}

/// Get a specific artists release
///
/// # Arguments
/// * `pool` - The database connection pool
/// * `user` - The current user
/// * `artist_slug` - The slug of the artist
/// * `release_slug` - The slug of the release
///
/// # Returns
/// The release for the artist
///
/// # Errors
/// If the artist cannot be found, return an error
/// If the user cannot be found, return an error
/// If the release cannot be found, return an error
#[cfg(feature = "server")]
pub async fn get_release_service(
    pool: &PgPool,
    user: Option<&User>,
    artist_slug: String,
    release_slug: String,
    include_deleted: bool,
) -> Result<ReleaseResult, AppError> {
    let artist = match Artist::get_by_slug(pool, artist_slug).await {
        Ok(artist) => artist,
        Err(e) => {
            let err = format!("get_release_service Error while getting artist: {e:?}");
            tracing::error!("{err}");
            Artist::default()
        }
    };

    // Opening one known row: a `label_owner` session reaches unpublished
    // and deleted releases too, so admins can open a row they deleted in
    // this session and restore it.
    let include_hidden =
        user.is_some_and(|current_user| current_user.permissions.contains("label_owner"));

    let release = Release::get_by_artist_and_record_label_and_slug(
        pool,
        artist.id,
        artist.label_id,
        release_slug.clone(),
        include_hidden,
    )
    .await
    .map_err(|e| {
        let err = format!("Error while getting releases: {e:?}");
        tracing::error!("{err}");
        AppError::internal(e)
    })?;
    let artists = release.get_artists(pool).await.map_err(|e| {
        let err = format!("Error while getting artists: {e:?}");
        tracing::error!("{err}");
        AppError::internal(e)
    })?;
    // The track list under a release is a listing, so it sits on the
    // visibility ladder (unlike the single release row above).
    let tracks = release
        .get_tracks(pool, visibility_for(user, include_deleted))
        .await
        .map_err(|e| {
            let err = format!("Error while getting tracks: {e:?}");
            tracing::error!("{err}");
            AppError::internal(e)
        })?;

    // The label's guest-play allowance rides along (the public release page
    // hands it to the global player). A label that cannot be read falls back
    // to the default allowance.
    let guest_plays = crate::models::record_label::RecordLabel::get_by_id(pool, release.label_id)
        .await
        .map_or_else(
            |e| {
                tracing::warn!("get_release_service: no label for guest plays: {e:?}");
                crate::models::record_label::RecordLabel::DEFAULT_GUEST_PLAYS
            },
            |label| label.guest_plays,
        );

    Ok(ReleaseResult {
        release,
        artists,
        tracks,
        guest_plays: Some(guest_plays),
    })
}

/// Create a new release
///
/// # Arguments
/// pool: `PgPool` - The database connection pool
/// user: Option<&User> - The user creating the release
/// `form`: `CreateReleaseForm` - The form to create the release
/// `include_deleted`: The admin "show deleted" toggle of the calling page;
///   it decides which tracks the Saved payload carries back
///
/// # Returns
/// Result<`ReleaseResult`, `AppError`> - The created release
///
/// # Errors
/// If the name is empty, return an error
/// If the release cannot be created, return an error
/// If the user does not have the required permissions, return an error
#[cfg(feature = "server")]
pub async fn create_release_service(
    pool: &PgPool,
    user: Option<&User>,
    form: CreateReleaseForm,
    include_deleted: bool,
) -> Result<ReleaseResult, AppError> {
    match user_with_permissions(user, vec!["admin", "label_owner"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }

    let release = Release::create(
        pool,
        form.name,
        form.description,
        form.primary_artist_id,
        form.catalogue_number,
        form.release_date,
        form.label_id,
        form.published_at,
    )
    .await
    .map_err(|e| {
        let err = format!("Error while creating release: {e:?}");
        tracing::error!("{err}");
        AppError::internal(e)
    })?;

    let artist_ids = form
        .artist_ids
        .split(',')
        .filter_map(|s| s.parse::<i64>().ok())
        .collect::<Vec<i64>>();
    release.set_artists(pool, artist_ids).await.map_err(|e| {
        let err = format!("Error while setting artists: {e:?}");
        tracing::error!("{err}");
        AppError::internal(e)
    })?;

    let artists = release.get_artists(pool).await.map_err(|e| {
        let err = format!("Error while getting artists: {e:?}");
        tracing::error!("{err}");
        AppError::internal(e)
    })?;
    // The Saved payload feeds the reactive editor's tracks table, so its
    // tracks sit on the same ladder the editor's page render used.
    let tracks = release
        .get_tracks(pool, visibility_for(user, include_deleted))
        .await
        .map_err(|e| {
            let err = format!("Error while getting tracks: {e:?}");
            tracing::error!("{err}");
            AppError::internal(e)
        })?;

    Ok(ReleaseResult {
        release,
        artists,
        tracks,
        // The editor round trips answer `None`: the guest allowance belongs
        // to the public release fetch (only that page hosts the player).
        guest_plays: None,
    })
}

/// Update a release
///
/// # Arguments
/// `pool`: `PgPool` - The database connection pool
/// `user`: `Option<&User>` - The user updating the release
/// `form`: `UpdateReleaseForm` - The form carrying the new field values
/// `include_deleted`: The admin "show deleted" toggle of the calling page;
///   it decides which tracks the Saved payload carries back
///
/// # Returns
/// Result<`ReleaseResult`, `AppError`> - The release as now stored
///
/// # Errors
/// If the release cannot be found, return an error
/// If the release cannot be updated, return an error
/// If the user does not have the required permissions, return an error
#[cfg(feature = "server")]
pub async fn update_release_service(
    pool: &PgPool,
    user: Option<&User>,
    form: UpdateReleaseForm,
    include_deleted: bool,
) -> Result<ReleaseResult, AppError> {
    match user_with_permissions(user, vec!["admin", "label_owner"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }

    let mut release = find_release_by_slug(pool, &form.slug).await?;

    release.name = form.name;
    release.description = form.description;
    release.primary_artist_id = form.primary_artist_id;
    release.catalogue_number = form.catalogue_number;
    release.release_date = form.release_date;
    release.published_at = form.published_at;

    release = release.update(pool).await.map_err(|e| {
        let err = format!("Error while updating release: {e:?}");
        tracing::error!("{err}");
        AppError::internal(e)
    })?;

    let artist_ids = form
        .artist_ids
        .split(',')
        .filter_map(|s| s.parse::<i64>().ok())
        .collect::<Vec<i64>>();
    release.set_artists(pool, artist_ids).await.map_err(|e| {
        let err = format!("Error while setting artists: {e:?}");
        tracing::error!("{err}");
        AppError::internal(e)
    })?;
    let artists = release.get_artists(pool).await.map_err(|e| {
        let err = format!("Error while getting artists: {e:?}");
        tracing::error!("{err}");
        AppError::internal(e)
    })?;
    // The Saved payload feeds the reactive editor's tracks table, so its
    // tracks sit on the same ladder the editor's page render used.
    let tracks = release
        .get_tracks(pool, visibility_for(user, include_deleted))
        .await
        .map_err(|e| {
            let err = format!("Error while getting tracks: {e:?}");
            tracing::error!("{err}");
            AppError::internal(e)
        })?;

    Ok(ReleaseResult {
        release,
        artists,
        tracks,
        guest_plays: None,
    })
}

/// Soft delete a release
///
/// # Arguments
/// pool: `PgPool` - The database connection pool
/// user: Option<&User> - The user deleting the release
/// slug: String - The slug of the release
/// `include_deleted`: The admin "show deleted" toggle of the calling page;
///   it decides which tracks the Saved payload carries back
///
/// # Returns
/// Result<`ReleaseResult`, `AppError`> - The deleted release
///
/// # Errors
/// If the release cannot be found, return an error
/// If the user does not have the required permissions, return an error
#[cfg(feature = "server")]
pub async fn delete_release_service(
    pool: &PgPool,
    user: Option<&User>,
    slug: String,
    include_deleted: bool,
) -> Result<ReleaseResult, AppError> {
    match user_with_permissions(user, vec!["admin", "label_owner"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }

    let release = find_release_by_slug(pool, &slug).await?;

    Ok(ReleaseResult {
        release: release.delete(pool).await.map_err(|e| {
            let err = format!("Error while deleting release: {e:?}");
            tracing::error!("{err}");
            AppError::internal(e)
        })?,
        artists: release.get_artists(pool).await.map_err(|e| {
            let err = format!("Error while getting artists: {e:?}");
            tracing::error!("{err}");
            AppError::internal(e)
        })?,
        tracks: release
            .get_tracks(pool, visibility_for(user, include_deleted))
            .await
            .map_err(|e| {
                let err = format!("Error while getting tracks: {e:?}");
                tracing::error!("{err}");
                AppError::internal(e)
            })?,
        guest_plays: None,
    })
}

/// Restore a soft deleted release
///
/// # Arguments
/// pool: `PgPool` - The database connection pool
/// user: Option<&User> - The user restoring the release
/// slug: String - The slug of the release
/// `include_deleted`: The admin "show deleted" toggle of the calling page;
///   it decides which tracks the Saved payload carries back
///
/// # Returns
/// Result<`ReleaseResult`, `AppError`> - The restored release
///
/// # Errors
/// If the release cannot be found, return an error
/// If the user does not have the required permissions, return an error
#[cfg(feature = "server")]
pub async fn restore_release_service(
    pool: &PgPool,
    user: Option<&User>,
    slug: String,
    include_deleted: bool,
) -> Result<ReleaseResult, AppError> {
    match user_with_permissions(user, vec!["admin", "label_owner"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }

    let mut release = find_release_by_slug(pool, &slug).await?;
    release.deleted_at = None;
    release.clone().update(pool).await.map_err(|e| {
        let err = format!("Error while restoring release: {e:?}");
        tracing::error!("{err}");
        AppError::internal(e)
    })?;

    Ok(ReleaseResult {
        release: release.clone(),
        artists: release.get_artists(pool).await.map_err(|e| {
            let err = format!("Error while getting artists: {e:?}");
            tracing::error!("{err}");
            AppError::internal(e)
        })?,
        tracks: release
            .get_tracks(pool, visibility_for(user, include_deleted))
            .await
            .map_err(|e| {
                let err = format!("Error while getting tracks: {e:?}");
                tracing::error!("{err}");
                AppError::internal(e)
            })?,
        guest_plays: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "server")]
    use crate::models::test_helpers::{
        create_test_artist, create_test_record_label, create_test_release, create_test_track,
        create_test_user, create_test_user_with_permissions,
    };

    #[sqlx::test]
    async fn test_get_releases_service_admin_user(pool: PgPool) {
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
        let mut unpublished_release = create_test_release(&pool, 2, Some(artist.clone()))
            .await
            .unwrap();
        unpublished_release.published_at = None;
        unpublished_release.release_date = None;
        unpublished_release.clone().update(&pool).await.unwrap();

        let releases = get_releases_service(&pool, Some(&user), artist.slug.clone(), false)
            .await
            .unwrap();

        assert_eq!(releases.releases.len(), 2);
        assert_eq!(releases.releases[0].id, unpublished_release.id);
        assert_eq!(releases.releases[1].id, release.id);
    }

    #[sqlx::test]
    async fn test_get_releases_service_no_user(pool: PgPool) {
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();
        let mut unpublished_release = create_test_release(&pool, 2, Some(artist.clone()))
            .await
            .unwrap();
        unpublished_release.published_at = None;
        unpublished_release.clone().update(&pool).await.unwrap();

        let releases = get_releases_service(&pool, None, artist.slug.clone(), false)
            .await
            .unwrap();

        assert_eq!(releases.releases.len(), 1);
        assert_eq!(releases.releases[0].id, release.id);
    }

    #[sqlx::test]
    async fn test_get_releases_service_anonymous_user(pool: PgPool) {
        let (user, _) = create_test_user(&pool, 1).await.unwrap().into_user(None);

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();
        let mut unpublished_release = create_test_release(&pool, 2, Some(artist.clone()))
            .await
            .unwrap();
        unpublished_release.published_at = None;
        unpublished_release.clone().update(&pool).await.unwrap();

        let releases = get_releases_service(&pool, Some(&user), artist.slug.clone(), false)
            .await
            .unwrap();

        assert_eq!(releases.releases.len(), 1);
        assert_eq!(releases.releases[0].id, release.id);
    }

    #[sqlx::test]
    async fn test_get_next_scheduled_release_service(pool: PgPool) {
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();
        let mut release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();
        release.release_date = Some(chrono::Utc::now() + chrono::Duration::days(1));
        release.clone().update(&pool).await.unwrap();

        let next_release =
            get_next_scheduled_release_service(&pool, Some(artist.id), record_label.id)
                .await
                .unwrap();

        assert!(next_release.is_some());
        assert_eq!(next_release.unwrap().release.id, release.id);
    }

    #[sqlx::test]
    async fn test_get_next_scheduled_release_service_no_artist(pool: PgPool) {
        let mut release = create_test_release(&pool, 1, None).await.unwrap();
        release.release_date = Some(chrono::Utc::now() + chrono::Duration::days(1));
        release.clone().update(&pool).await.unwrap();

        let next_release = get_next_scheduled_release_service(&pool, None, release.label_id)
            .await
            .unwrap();

        assert!(next_release.is_some());
        assert_eq!(next_release.unwrap().release.id, release.id);
    }

    #[sqlx::test]
    async fn test_get_release_service(pool: PgPool) {
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

        let release_result = get_release_service(
            &pool,
            Some(&user),
            artist.slug.clone(),
            release.slug.clone(),
            false,
        )
        .await
        .unwrap();

        assert_eq!(release_result.release.id, release.id);
        assert_eq!(release_result.tracks.len(), 1);
        assert_eq!(release_result.tracks[0].track.id, track.id);
        assert_eq!(release_result.artists.len(), 1);
        assert_eq!(release_result.artists[0].id, artist.id);
    }

    #[sqlx::test]
    async fn test_get_release_service_no_permission(pool: PgPool) {
        let permissions = vec![];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();
        let mut unpublished_release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();
        unpublished_release.published_at = None;
        unpublished_release.clone().update(&pool).await.unwrap();

        let release_result = get_release_service(
            &pool,
            Some(&user),
            artist.slug.clone(),
            unpublished_release.slug.clone(),
            false,
        )
        .await;
        assert!(release_result.is_err());
        assert_eq!(
            release_result.unwrap_err().to_string(),
            "Could not find release test-release-1 for artist with id 1 and record label with id 1.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_get_release_service_no_iser(pool: PgPool) {
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();
        let mut unpublished_release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();
        unpublished_release.published_at = None;
        unpublished_release.clone().update(&pool).await.unwrap();

        let release_result = get_release_service(
            &pool,
            None,
            artist.slug.clone(),
            unpublished_release.slug.clone(),
            false,
        )
        .await;
        assert!(release_result.is_err());
        assert_eq!(
            release_result.unwrap_err().to_string(),
            "Could not find release test-release-1 for artist with id 1 and record label with id 1.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_get_releases_service_show_deleted_toggle(pool: PgPool) {
        let permissions = vec!["label_owner"];
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
        let deleted_release = create_test_release(&pool, 2, Some(artist.clone()))
            .await
            .unwrap();
        deleted_release.clone().delete(&pool).await.unwrap();

        // Toggle off: a `label_owner` listing excludes the deleted release.
        let releases = get_releases_service(&pool, Some(&user), artist.slug.clone(), false)
            .await
            .unwrap();
        assert_eq!(releases.releases.len(), 1);
        assert_eq!(releases.releases[0].id, release.id);

        // Toggle on: the deleted release joins the listing.
        let releases = get_releases_service(&pool, Some(&user), artist.slug.clone(), true)
            .await
            .unwrap();
        assert_eq!(releases.releases.len(), 2);

        // Guests never see deleted rows, even with the toggle on.
        let releases = get_releases_service(&pool, None, artist.slug.clone(), true)
            .await
            .unwrap();
        assert_eq!(releases.releases.len(), 1);
        assert_eq!(releases.releases[0].id, release.id);
    }

    #[sqlx::test]
    async fn test_get_release_service_tracks_follow_visibility(pool: PgPool) {
        let permissions = vec!["label_owner"];
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
        let published_track =
            create_test_track(&pool, 1, Some(release.clone()), Some(artist.clone()))
                .await
                .unwrap();
        let deleted_track =
            create_test_track(&pool, 2, Some(release.clone()), Some(artist.clone()))
                .await
                .unwrap();
        deleted_track.clone().delete(&pool).await.unwrap();

        // Staff default: the live track is in, the deleted track out.
        let result = get_release_service(
            &pool,
            Some(&user),
            artist.slug.clone(),
            release.slug.clone(),
            false,
        )
        .await
        .unwrap();
        assert_eq!(result.tracks.len(), 1);
        assert_eq!(result.tracks[0].track.id, published_track.id);

        // Toggle on: the deleted track appears as well.
        let result = get_release_service(
            &pool,
            Some(&user),
            artist.slug.clone(),
            release.slug.clone(),
            true,
        )
        .await
        .unwrap();
        assert_eq!(result.tracks.len(), 2);

        // Guest: only published tracks — deleted (and unpublished) tracks
        // never reach the public release page.
        let result =
            get_release_service(&pool, None, artist.slug.clone(), release.slug.clone(), true)
                .await
                .unwrap();
        assert_eq!(result.tracks.len(), 1);
        assert_eq!(result.tracks[0].track.id, published_track.id);
    }

    #[sqlx::test]
    async fn test_create_release_service(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user: User = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();

        let form = CreateReleaseForm {
            name: "Test Release".to_string(),
            description: "Test Release Description".to_string(),
            primary_artist_id: artist.id,
            catalogue_number: "TEST-123".to_string(),
            release_date: Some(chrono::Utc::now()),
            label_id: record_label.id,
            published_at: Some(chrono::Utc::now()),
            artist_ids: artist.id.to_string(),
        };

        let release_result = create_release_service(&pool, Some(&user), form.clone(), false)
            .await
            .unwrap();

        assert_eq!(release_result.release.name, "Test Release");
        assert_eq!(
            release_result.release.description,
            "Test Release Description"
        );
        assert_eq!(release_result.release.primary_artist_id, artist.id);
        assert_eq!(release_result.release.catalogue_number, "TEST-123");
        assert!(release_result.release.release_date.is_some());
        assert_eq!(release_result.release.label_id, record_label.id);
        assert!(release_result.release.published_at.is_some());
        assert_eq!(release_result.artists.len(), 1);
        assert_eq!(release_result.artists[0].id, artist.id);
    }

    #[sqlx::test]
    async fn test_create_release_service_no_permision(pool: PgPool) {
        let permissions = vec![];
        let user: User = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();

        let form = CreateReleaseForm {
            name: "Test Release".to_string(),
            description: "Test Release Description".to_string(),
            primary_artist_id: artist.id,
            catalogue_number: "TEST-123".to_string(),
            release_date: Some(chrono::Utc::now()),
            label_id: record_label.id,
            published_at: Some(chrono::Utc::now()),
            artist_ids: artist.id.to_string(),
        };

        let release_result = create_release_service(&pool, Some(&user), form, false).await;
        assert!(release_result.is_err());
        assert_eq!(
            release_result.unwrap_err().to_string(),
            "You do not have permission.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_update_release_service_no_permission(pool: PgPool) {
        let permissions = vec![];
        let user: User = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();

        let form = CreateReleaseForm {
            name: "Test Release".to_string(),
            description: "Test Release Description".to_string(),
            primary_artist_id: artist.id,
            catalogue_number: "TEST-123".to_string(),
            release_date: Some(chrono::Utc::now()),
            label_id: record_label.id,
            published_at: Some(chrono::Utc::now()),
            artist_ids: artist.id.to_string(),
        };

        let release_result = create_release_service(&pool, Some(&user), form, false).await;
        assert!(release_result.is_err());
        assert_eq!(
            release_result.unwrap_err().to_string(),
            "You do not have permission.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_update_release_service(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user: User = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();

        let form = CreateReleaseForm {
            name: "Test Release".to_string(),
            description: "Test Release Description".to_string(),
            catalogue_number: "TEST-123".to_string(),
            primary_artist_id: artist.id,
            release_date: Some(chrono::Utc::now()),
            label_id: record_label.id,
            published_at: Some(chrono::Utc::now()),
            artist_ids: artist.id.to_string(),
        };

        let release_result = create_release_service(&pool, Some(&user), form, false).await;
        assert!(release_result.is_ok());
        let release = release_result.unwrap();

        let update_form = UpdateReleaseForm {
            name: "Updated Release".to_string(),
            slug: "test-release".to_string(),
            description: "Updated Release Description".to_string(),
            primary_artist_id: artist.id,
            catalogue_number: "UPDATED-123".to_string(),
            release_date: Some(chrono::Utc::now()),
            label_id: record_label.id,
            published_at: Some(chrono::Utc::now()),
            artist_ids: artist.id.to_string(),
        };

        let update_result =
            update_release_service(&pool, Some(&user), update_form.clone(), false).await;
        assert!(update_result.is_ok());
        let updated_release = update_result.unwrap();

        assert_eq!(release.release.id, updated_release.release.id);
        assert_eq!(updated_release.release.name, "Updated Release");
        assert_eq!(
            updated_release.release.description,
            "Updated Release Description"
        );
        assert_eq!(updated_release.release.primary_artist_id, artist.id);
        assert_eq!(updated_release.release.catalogue_number, "UPDATED-123");
        assert!(updated_release.release.release_date.is_some());
        assert_eq!(updated_release.release.label_id, record_label.id);
        assert!(updated_release.release.published_at.is_some());
        assert_eq!(updated_release.artists, vec![artist]);
    }

    #[sqlx::test]
    pub fn delete_release(pool: sqlx::PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user: User = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();

        let form = CreateReleaseForm {
            name: "Test Release".to_string(),
            description: "Test Release Description".to_string(),
            primary_artist_id: artist.id,
            catalogue_number: "TEST-123".to_string(),
            release_date: Some(chrono::Utc::now()),
            label_id: record_label.id,
            published_at: Some(chrono::Utc::now()),
            artist_ids: artist.id.to_string(),
        };

        let release_result = create_release_service(&pool, Some(&user), form, false).await;
        assert!(release_result.is_ok());
        let release = release_result.unwrap();

        let delete_result =
            delete_release_service(&pool, Some(&user), release.release.slug.clone(), false).await;
        assert!(delete_result.is_ok());

        let get_result =
            get_release_service(&pool, Some(&user), artist.slug, release.release.slug, false).await;
        assert!(get_result.is_ok());
        assert!(get_result.unwrap().release.deleted_at.is_some());
    }

    #[sqlx::test]
    pub fn delete_release_not_found(pool: sqlx::PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let delete_result =
            delete_release_service(&pool, Some(&user), "not-found".to_string(), false).await;
        assert!(delete_result.is_err());
        assert_eq!(
            delete_result.unwrap_err().to_string(),
            "Could not find release with slug not-found.".to_string()
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

        let delete_result =
            delete_release_service(&pool, Some(&user), release.slug.clone(), false).await;
        assert_eq!(
            delete_result.unwrap_err().to_string(),
            "You do not have permission.".to_string()
        );

        let get_result =
            get_release_service(&pool, Some(&user), artist.slug, release.slug, false).await;
        assert!(get_result.is_ok());
    }

    #[sqlx::test]
    async fn test_restore_release_service(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let release = create_test_release(&pool, 1, None).await.unwrap();
        let release = release.delete(&pool).await.unwrap();
        let restored_release =
            restore_release_service(&pool, Some(&user), release.slug.clone(), false)
                .await
                .unwrap();

        assert!(restored_release.release.deleted_at.is_none());
    }

    #[sqlx::test]
    async fn test_restore_release_service_no_release(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let restored_release =
            restore_release_service(&pool, Some(&user), "missing".to_string(), false).await;
        assert!(restored_release.is_err());
        assert_eq!(
            restored_release.unwrap_err().to_string(),
            "Could not find release with slug missing.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_restore_release_service_no_user(pool: PgPool) {
        let release = create_test_release(&pool, 1, None).await.unwrap();
        let release = release.delete(&pool).await.unwrap();
        let restored_release =
            restore_release_service(&pool, Some(&User::default()), release.slug.clone(), false)
                .await;
        assert!(restored_release.is_err());
        assert_eq!(
            restored_release.unwrap_err().to_string(),
            "You must be logged in to view this page.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_restore_release_service_no_permissions(pool: PgPool) {
        let user = create_test_user_with_permissions(&pool, 1, vec![])
            .await
            .unwrap();
        let release = create_test_release(&pool, 1, None).await.unwrap();
        let release = release.delete(&pool).await.unwrap();
        let restored_release =
            restore_release_service(&pool, Some(&user), release.slug.clone(), false).await;
        assert!(restored_release.is_err());
        assert_eq!(
            restored_release.unwrap_err().to_string(),
            "You do not have permission.".to_string()
        );
    }
}
