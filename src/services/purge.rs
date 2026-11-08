//! Housekeeping: permanently remove rows whose 90-day soft-delete window
//! has passed.
//!
//! The soft-delete flows (`.../delete`) keep rows restorable; this service
//! is the other half, normally fired from the admin dashboard's
//! "Housekeeping" card. It answers with per-table counts, both for a
//! preview (`confirm = false`, which changes nothing) and for the real
//! purge (`confirm = true`):
//!
//! ```text
//! preview:  PurgeResult { tracks: 4, releases: 2, artists: 1, pages: 0, skipped_artists: 3 }
//! confirm:  same counts, and the rows are gone.
//! ```
//!
//! Rules (all inside one transaction, so a purge either fully applies or
//! not at all):
//!
//! - Tracks, releases and pages past their window are removed outright.
//! - A purged release cascades its tracks (whenever they were deleted, or
//!   not deleted at all): a track is only ever reachable through its
//!   release, and the `tracks.release_id` foreign key is `RESTRICT`, so
//!   the tracks have to go first anyway.
//! - Artists go last, once their doomed releases/tracks are gone. A
//!   *surviving* row that still names a past-window artist as its
//!   `primary_artist_id` blocks that artist's removal: the models read
//!   that column as a plain `i64`, and the `ON DELETE SET NULL` foreign
//!   key would leave those rows undecodable. Those artists are counted
//!   in `skipped_artists`; deleting the referencing rows first frees them
//!   on the next run.
//! - Join rows (`release_artists`, `track_artists`, `release_tracks`)
//!   follow their parent through `ON DELETE CASCADE`.
//! - Uploads referenced by purged rows stay on disk; cleaning the file
//!   store is a separate follow-up.

use chrono::Utc;
use sqlx::{PgPool, Postgres, Transaction};

use super::authentication_helpers::user_with_permissions;
use crate::api_types::purge::{PURGE_AFTER_DAYS, PurgeResult};
use crate::error::AppError;
use crate::models::auth::User;

/// Remove every soft-deleted row older than [`PURGE_AFTER_DAYS`] days.
///
/// With `confirm = false` the whole purge runs and is rolled back, so the
/// counts a preview reports are the counts the real purge will produce
/// (same statements, same order — the preview cannot drift).
///
/// # Arguments
/// * `pool` - The database connection pool
/// * `user` - The current user; must hold the `admin` permission
/// * `confirm` - `false` previews, `true` commits
///
/// # Returns
/// A [`PurgeResult`] with the per-table counts (removed, or would-be
/// removed) plus the artists skipped because live rows still reference
/// them.
///
/// # Errors
/// If the user does not hold `admin`, or the transaction cannot run,
/// return a `AppError`.
#[cfg(feature = "server")]
pub async fn purge_deleted_service(
    pool: &PgPool,
    user: Option<&User>,
    confirm: bool,
) -> Result<PurgeResult, AppError> {
    // Full admins only: unlike the soft-deleting services, `label_owner`
    // is not enough to wipe data for good.
    match user_with_permissions(user, vec!["admin"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }

    let cutoff = Utc::now() - chrono::Duration::days(PURGE_AFTER_DAYS);

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!("purge transaction could not start: {e:?}");
        AppError::internal("Could not run the purge, try again later")
    })?;

    // Tracks deleted on their own, then the doomed releases' cascaded
    // tracks, then the releases themselves (the FK above is RESTRICT).
    let old_tracks = delete_past_window(&mut tx, "tracks", cutoff)
        .await
        .map_err(purge_failure)?;
    let cascaded_tracks = sqlx::query(
        "DELETE FROM tracks WHERE release_id IN
         (SELECT id FROM releases WHERE deleted_at IS NOT NULL AND deleted_at < $1)",
    )
    .bind(cutoff)
    .execute(&mut *tx)
    .await
    .map_err(purge_failure)?
    .rows_affected();
    let releases = delete_past_window(&mut tx, "releases", cutoff)
        .await
        .map_err(purge_failure)?;
    let pages = delete_past_window(&mut tx, "pages", cutoff)
        .await
        .map_err(purge_failure)?;

    // Artists last (their doomed rows are gone by now). `skipped` covers
    // the artists a surviving row still pins as its primary artist.
    let past_window_artists = past_window_count(&mut tx, cutoff)
        .await
        .map_err(purge_failure)?;
    let artists = sqlx::query(
        "DELETE FROM artists
         WHERE deleted_at IS NOT NULL AND deleted_at < $1
           AND NOT EXISTS (SELECT 1 FROM releases r WHERE r.primary_artist_id = artists.id)
           AND NOT EXISTS (SELECT 1 FROM tracks t WHERE t.primary_artist_id = artists.id)",
    )
    .bind(cutoff)
    .execute(&mut *tx)
    .await
    .map_err(purge_failure)?
    .rows_affected();
    let skipped_artists = past_window_artists.saturating_sub(artists);

    if confirm {
        tx.commit().await.map_err(purge_failure)?;
    } else {
        // The preview: the deletes above ran (so the counts and the FK
        // ordering are exactly the real purge's) and are now undone.
        tx.rollback().await.map_err(purge_failure)?;
    }

    Ok(PurgeResult {
        tracks: old_tracks + cascaded_tracks,
        releases,
        artists,
        pages,
        skipped_artists,
    })
}

/// `DELETE` the rows of `table` whose `deleted_at` predates `cutoff`.
///
/// Only the literal table names used below ever reach the format string.
async fn delete_past_window(
    tx: &mut Transaction<'_, Postgres>,
    table: &'static str,
    cutoff: chrono::DateTime<Utc>,
) -> sqlx::Result<u64> {
    let sql = format!("DELETE FROM {table} WHERE deleted_at IS NOT NULL AND deleted_at < $1");
    let removed = sqlx::query(&sql)
        .bind(cutoff)
        .execute(&mut **tx)
        .await?
        .rows_affected();
    Ok(removed)
}

/// How many artists sit past the soft-delete window, purged or not.
async fn past_window_count(
    tx: &mut Transaction<'_, Postgres>,
    cutoff: chrono::DateTime<Utc>,
) -> sqlx::Result<u64> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM artists WHERE deleted_at IS NOT NULL AND deleted_at < $1",
    )
    .bind(cutoff)
    .fetch_one(&mut **tx)
    .await?;
    Ok(count.max(0).cast_unsigned())
}

/// Database failures keep their detail in the log and answer as one fixed
/// retry message; the form and twin surfaces turn this into `?bad=server`.
#[expect(
    clippy::needless_pass_by_value,
    reason = "call sites are `map_err(purge_failure)`, which hands over the error by value"
)]
fn purge_failure(e: sqlx::Error) -> AppError {
    tracing::error!("purge failed: {e:?}");
    AppError::internal("Could not run the purge, try again later")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::test_helpers::{
        create_test_artist, create_test_page, create_test_record_label, create_test_release,
        create_test_track, create_test_user_with_permissions,
    };

    /// Move a soft-deleted row's `deleted_at` back past the window.
    /// `update` persists `deleted_at`, so the helpers' `delete()` (which
    /// stamps `now()`) can be aged deliberately.
    async fn age_artist(pool: &PgPool, artist: crate::models::artist::Artist) {
        let mut artist = artist.delete(pool).await.expect("delete artist");
        artist.deleted_at = Some(Utc::now() - chrono::Duration::days(PURGE_AFTER_DAYS + 1));
        artist.update(pool).await.expect("age artist");
    }

    async fn age_release(pool: &PgPool, release: crate::models::release::Release) {
        let mut release = release.delete(pool).await.expect("delete release");
        release.deleted_at = Some(Utc::now() - chrono::Duration::days(PURGE_AFTER_DAYS + 1));
        release.update(pool).await.expect("age release");
    }

    async fn age_page(pool: &PgPool, page: crate::models::page::Page) {
        let mut page = page.delete(pool).await.expect("delete page");
        page.deleted_at = Some(Utc::now() - chrono::Duration::days(PURGE_AFTER_DAYS + 1));
        page.update(pool).await.expect("age page");
    }

    async fn age_track(pool: &PgPool, track: crate::models::track::Track) {
        let mut track = track.delete(pool).await.expect("delete track");
        track.deleted_at = Some(Utc::now() - chrono::Duration::days(PURGE_AFTER_DAYS + 1));
        track.update(pool).await.expect("age track");
    }

    #[sqlx::test]
    async fn purge_requires_admin(pool: PgPool) {
        let permissions = vec!["label_owner"];
        let owner = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .expect("label_owner user");

        let result = purge_deleted_service(&pool, Some(&owner), true).await;
        assert!(result.is_err(), "label_owner alone may not purge");

        let guest = purge_deleted_service(&pool, None, true).await;
        assert!(guest.is_err(), "guests may not purge");
    }

    #[sqlx::test]
    async fn preview_changes_nothing(pool: PgPool) {
        let admin = create_test_user_with_permissions(&pool, 1, vec!["admin"])
            .await
            .expect("admin user");
        let label = create_test_record_label(&pool, 1).await.expect("label");
        let artist = create_test_artist(&pool, 1, Some(label.clone()))
            .await
            .expect("artist");
        age_artist(&pool, artist.clone()).await;

        let preview = purge_deleted_service(&pool, Some(&admin), false)
            .await
            .expect("preview runs");
        assert_eq!(preview.artists, 1);
        assert_eq!(preview.skipped_artists, 0);

        // The rollback left the row where it was, restorable as before.
        let still_there = crate::models::artist::Artist::get_by_id(&pool, artist.id)
            .await
            .expect("fetch artist");
        assert!(still_there.deleted_at.is_some());

        // A confirm on the same state now really removes it.
        let purged = purge_deleted_service(&pool, Some(&admin), true)
            .await
            .expect("purge runs");
        assert_eq!(purged.artists, 1);
        let gone = crate::models::artist::Artist::find_by_id(&pool, artist.id)
            .await
            .expect("fetch artist");
        assert!(gone.is_none(), "the confirmed purge removed the row");
    }

    #[sqlx::test]
    async fn purge_removes_past_window_rows_and_cascades(pool: PgPool) {
        let admin = create_test_user_with_permissions(&pool, 1, vec!["admin"])
            .await
            .expect("admin user");
        let label = create_test_record_label(&pool, 1).await.expect("label");
        let artist = create_test_artist(&pool, 1, Some(label.clone()))
            .await
            .expect("artist");

        // A release past its window, with one live track and one
        // soft-deleted-just-recently track: both cascade, the release goes.
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .expect("release");
        let _live_track = create_test_track(&pool, 1, Some(release.clone()), Some(artist.clone()))
            .await
            .expect("live track");
        let recent_track = create_test_track(&pool, 2, Some(release.clone()), Some(artist.clone()))
            .await
            .expect("recent track");
        recent_track.delete(&pool).await.expect("soft-delete track");
        age_release(&pool, release.clone()).await;

        // A track past its window under a release that survives.
        let keeper_release = create_test_release(&pool, 2, Some(artist.clone()))
            .await
            .expect("keeper release");
        let old_track =
            create_test_track(&pool, 3, Some(keeper_release.clone()), Some(artist.clone()))
                .await
                .expect("old track");
        age_track(&pool, old_track.clone()).await;

        // A page past its window.
        let page = create_test_page(&pool, 1, Some(label.clone()))
            .await
            .expect("page");
        age_page(&pool, page.clone()).await;

        // The artist is past its window, but the surviving keeper_release
        // names it as primary artist, which blocks the removal.
        age_artist(&pool, artist.clone()).await;

        let result = purge_deleted_service(&pool, Some(&admin), true)
            .await
            .expect("purge runs");

        // Two cascaded tracks (one live!) and one old soft-deleted track.
        assert_eq!(result.tracks, 3);
        assert_eq!(result.releases, 1);
        assert_eq!(result.pages, 1);

        for track_slug in ["test-track-1", "test-track-2", "test-track-3"] {
            let track = crate::models::track::Track::find_by_slug(&pool, track_slug.to_string())
                .await
                .expect("fetch track");
            assert!(track.is_none(), "track {track_slug} should be gone");
        }
        assert!(
            crate::models::release::Release::find_by_slug(&pool, release.slug.clone())
                .await
                .expect("fetch release")
                .is_none()
        );
        assert!(
            crate::models::page::Page::find_by_slug(&pool, page.slug.clone(), true)
                .await
                .expect("fetch page")
                .is_none()
        );

        // The past-window artist was skipped, not purged: removing it
        // would leave keeper_release with a null primary artist.
        assert_eq!(result.artists, 0);
        assert_eq!(result.skipped_artists, 1);
        let blocked_artist = crate::models::artist::Artist::get_by_id(&pool, artist.id)
            .await
            .expect("fetch artist");
        assert!(
            blocked_artist.deleted_at.is_some(),
            "the blocked artist stays soft-deleted"
        );
    }

    #[sqlx::test]
    async fn artist_purges_once_its_rows_go_first(pool: PgPool) {
        let admin = create_test_user_with_permissions(&pool, 1, vec!["admin"])
            .await
            .expect("admin user");
        let label = create_test_record_label(&pool, 1).await.expect("label");
        let artist = create_test_artist(&pool, 1, Some(label.clone()))
            .await
            .expect("artist");
        // The artist and its only release are both past the window; the
        // release goes first, which frees the artist.
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .expect("release");
        age_release(&pool, release).await;
        age_artist(&pool, artist.clone()).await;

        let result = purge_deleted_service(&pool, Some(&admin), true)
            .await
            .expect("purge runs");
        assert_eq!(result.releases, 1);
        assert_eq!(result.artists, 1);
        assert_eq!(result.skipped_artists, 0);
        assert!(
            crate::models::artist::Artist::find_by_id(&pool, artist.id)
                .await
                .expect("fetch artist")
                .is_none()
        );
    }

    #[sqlx::test]
    async fn rows_inside_the_window_survive(pool: PgPool) {
        let admin = create_test_user_with_permissions(&pool, 1, vec!["admin"])
            .await
            .expect("admin user");
        let label = create_test_record_label(&pool, 1).await.expect("label");
        let artist = create_test_artist(&pool, 1, Some(label.clone()))
            .await
            .expect("artist");
        // Deleted (at `now()`), but not yet past the 90-day window.
        artist.delete(&pool).await.expect("soft-delete artist");
        let page = create_test_page(&pool, 1, Some(label)).await.expect("page");
        page.delete(&pool).await.expect("soft-delete page");

        let result = purge_deleted_service(&pool, Some(&admin), true)
            .await
            .expect("purge runs");
        assert_eq!(result.total(), 0);
        assert_eq!(result.skipped_artists, 0);

        let artist = crate::models::artist::Artist::get_by_id(&pool, artist.id)
            .await
            .expect("fetch artist");
        assert!(artist.deleted_at.is_some());
    }
}
