//! Housekeeping: permanently remove rows whose 90-day soft-delete window
//! has passed, and the uploads no surviving row will ever show again.
//!
//! The soft-delete flows (`.../delete`) keep rows restorable; this service
//! is the other half, normally fired from the admin dashboard's
//! "Housekeeping" card. It names the rows table by table, both for a
//! preview (`confirm = false`, which only reads) and for the real purge
//! (`confirm = true`):
//!
//! ```text
//! preview:  PurgeResult { pages: ["Test Page 1"], skipped_artists: ["Test Artist 2"],
//!           files: ["releases/old-cover.png"], ..}
//! confirm:  the same names, the rows gone, and the named files removed.
//! ```
//!
//! The preview and the purge share their SQL predicates (the
//! `PAST_WINDOW`/`DOOMED_TRACKS`/`ARTIST_REFS` fragments below): the
//! preview's SELECTs list exactly the rows the purge's
//! `DELETE ... RETURNING name` statements remove, so the card can show
//! each doomed row by name without a throwaway write transaction. The
//! `ORPHAN_UPLOADS` query is shared the same way: it lists exactly the
//! upload files the confirmed run removes from disk.
//!
//! Rules (the purge runs inside one transaction, so the database half
//! either fully applies or not at all):
//!
//! - Tracks, releases and pages past their window are removed outright.
//! - A purged release cascades its tracks (whenever they were deleted, or
//!   not deleted at all): a track is only ever reachable through its
//!   release, and the `tracks.release_id` foreign key is `RESTRICT`, so
//!   the tracks have to go first anyway.
//! - Artists go last, once their doomed releases/tracks are accounted
//!   for. A *surviving* row that still names a past-window artist as its
//!   `primary_artist_id` blocks that artist's removal: the models read
//!   that column as a plain `i64`, and the `ON DELETE SET NULL` foreign
//!   key would leave those rows undecodable. Those artists are named in
//!   `skipped_artists`; deleting the referencing rows first frees them
//!   on the next run.
//! - Join rows (`release_artists`, `track_artists`, `release_tracks`)
//!   follow their parent through `ON DELETE CASCADE`.
//! - An upload file goes with the rows when none of the rows it could
//!   still serve survives the purge (the same file can be named by more
//!   than one row, live or restorable). The sweep also collects files the
//!   *database stopped naming long ago*: replaced avatars/covers/audio
//!   leave their previous file on disk, and purges predating this service
//!   left rows' files behind. Every file under the uploads root's
//!   `artists`, `releases`, `tracks` and `avatars` directories that no
//!   row names at all is orphaned and goes too — the sweep skips `tmp/`
//!   (upload staging) and anything in a subdirectory of its own.
//!   The preview names each such file `<dir>/<file>` and the confirmed
//!   run removes it — including files a previous half-run already
//!   deleted, so re-runs converge. A file-only purge (rows all inside
//!   their windows, dead files on disk) is still worth running. The files
//!   are removed after the transaction commits; a file the disk refuses
//!   to give up is logged and left out of the result, never a failed
//!   purge.
//! - Without a configured uploads root, the purge only removes rows and
//!   the `files` lists stay empty, preview included.

use chrono::Utc;
use sqlx::{PgPool, Postgres, Row};

use super::authentication_helpers::user_with_permissions;
use crate::api_types::purge::{PURGE_AFTER_DAYS, PurgeResult};
use crate::error::AppError;
use crate::models::auth::User;

/// The rows past their soft-delete window: the one predicate every
/// preview SELECT and purge DELETE shares, so a preview can only ever
/// list rows a confirmed run would actually remove.
const PAST_WINDOW: &str = "deleted_at IS NOT NULL AND deleted_at < $1";

/// The tracks the purge removes: their own past-window deletions, plus
/// every track under a past-window release.
///
/// A track is only ever reachable through its release, and tracks must go
/// before releases anyway (the `tracks.release_id` foreign key is
/// `RESTRICT`).
const DOOMED_TRACKS: &str = "(deleted_at IS NOT NULL AND deleted_at < $1
     OR release_id IN (SELECT id FROM releases WHERE deleted_at IS NOT NULL AND deleted_at < $1))";

/// A live reference that blocks a past-window artist's removal (correlated
/// on `artists.id`): a release the purge would keep, or a track the purge
/// would keep.
///
/// A kept track is one not itself past its window and not under a
/// past-window release. Same text at preview and at purge time, because it
/// names surviving rows directly instead of relying on what has been
/// deleted so far.
const ARTIST_REFS: &str = "(EXISTS (SELECT 1 FROM releases r
          WHERE r.primary_artist_id = artists.id
            AND NOT (r.deleted_at IS NOT NULL AND r.deleted_at < $1))
      OR EXISTS (SELECT 1 FROM tracks t
          WHERE t.primary_artist_id = artists.id
            AND NOT (t.deleted_at IS NOT NULL AND t.deleted_at < $1)
            AND t.release_id NOT IN
                (SELECT id FROM releases WHERE deleted_at IS NOT NULL AND deleted_at < $1)))";

/// The uploads subdirectories, matching the URLs the models build
/// (`artist.rs`, `release.rs`, `track.rs`: `/uploads/<dir>/<file>`), plus
/// the `avatars` directory the profile editor writes to.
///
/// Rows never name `avatars` in the row-derived orphan pass (user rows are
/// never purged), but users replace avatars, so replaced files land there
/// and the unreferenced sweep picks them up. `tmp` (upload staging) is
/// deliberately absent: it is never listed for removal, in-flight or
/// otherwise.
const UPLOAD_DIRS: [&str; 4] = ["artists", "releases", "tracks", "avatars"];

/// Upload files the purge orphans: `<dir, file>` pairs named by a row it
/// removes and by no row it keeps.
///
/// A kept artist (a past-window artist an `ARTIST_REFS` row pins) keeps
/// its own image, and a name shared with any surviving row — including a
/// restorable row inside its window — survives. One statement, shared by
/// preview and purge, so the card's file list is exactly the removal list;
/// `EXCEPT` also folds a name two doomed rows share into one.
fn orphan_uploads_sql() -> String {
    format!(
        "WITH doomed AS (
            SELECT 'artists' AS dir, primary_image AS file FROM artists
              WHERE {PAST_WINDOW} AND NOT {ARTIST_REFS} AND primary_image IS NOT NULL
            UNION SELECT 'releases', primary_image FROM releases
              WHERE {PAST_WINDOW} AND primary_image IS NOT NULL
            UNION SELECT 'tracks', primary_image FROM tracks
              WHERE {DOOMED_TRACKS} AND primary_image IS NOT NULL
            UNION SELECT 'tracks', audio FROM tracks
              WHERE {DOOMED_TRACKS} AND audio IS NOT NULL),
         kept AS (
            SELECT 'artists' AS dir, primary_image AS file FROM artists
              WHERE NOT ({PAST_WINDOW} AND NOT {ARTIST_REFS}) AND primary_image IS NOT NULL
            UNION SELECT 'releases', primary_image FROM releases
              WHERE NOT ({PAST_WINDOW}) AND primary_image IS NOT NULL
            UNION SELECT 'tracks', primary_image FROM tracks
              WHERE NOT ({DOOMED_TRACKS}) AND primary_image IS NOT NULL
            UNION SELECT 'tracks', audio FROM tracks
              WHERE NOT ({DOOMED_TRACKS}) AND audio IS NOT NULL)
         SELECT dir, file FROM doomed EXCEPT SELECT dir, file FROM kept"
    )
}

/// Remove every soft-deleted row older than [`PURGE_AFTER_DAYS`] days, and
/// the upload files none of the surviving rows will ever show again, or
/// just name the rows and files such a purge would remove.
///
/// # Arguments
/// * `pool` - The database connection pool
/// * `user` - The current user; must hold the `admin` permission
/// * `confirm` - `false` previews (reads only), `true` commits
/// * `upload_root` - The uploads directory, to name and remove orphaned
///   files. `None` (a server with no uploads configured) keeps the `files`
///   lists empty even for a preview, which promises nothing it cannot do.
///
/// # Returns
/// A [`PurgeResult`] naming the per-table rows (removed, or would-be
/// removed), the artists skipped because live rows still reference them,
/// and the orphaned upload files (`<dir>/<file>`) removed, or would-be
/// removed.
///
/// # Errors
/// If the user does not hold `admin`, or the queries cannot run, return
/// an `AppError`.
#[cfg(feature = "server")]
pub async fn purge_deleted_service(
    pool: &PgPool,
    user: Option<&User>,
    confirm: bool,
    upload_root: Option<&std::path::Path>,
) -> Result<PurgeResult, AppError> {
    // Full admins only: unlike the soft-deleting services, `label_owner`
    // is not enough to wipe data for good.
    match user_with_permissions(user, vec!["admin"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }

    let cutoff = Utc::now() - chrono::Duration::days(PURGE_AFTER_DAYS);

    if !confirm {
        let preview = preview(pool, cutoff, upload_root)
            .await
            .map_err(purge_failure)?;
        return Ok(preview);
    }

    // Files the database stopped naming long ago (replaced uploads, and
    // rows purged before this service existed) cannot be found from the
    // doomed rows — sweep them off disk now, while every row is still
    // here to compare against.
    let disk_orphans = match upload_root {
        Some(root) => unreferenced_uploads(pool, root)
            .await
            .map_err(purge_failure)?,
        None => Vec::new(),
    };

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!("purge transaction could not start: {e:?}");
        AppError::internal("Could not run the purge, try again later")
    })?;

    // Row-derived orphans next, while every row is still here to compare
    // against (the DELETEs below would shrink the kept set).
    let orphans = if upload_root.is_some() {
        let row_orphans = orphan_uploads(&mut *tx, cutoff)
            .await
            .map_err(purge_failure)?;
        merge_orphans(row_orphans, disk_orphans)
    } else {
        Vec::new()
    };

    // Tracks first (their own window and the doomed releases' cascades,
    // one statement), then the releases the FK made them wait for.
    let tracks = sorted_names(
        &mut *tx,
        &format!("DELETE FROM tracks WHERE {DOOMED_TRACKS} RETURNING name"),
        cutoff,
    )
    .await
    .map_err(purge_failure)?;
    let releases = sorted_names(
        &mut *tx,
        &format!("DELETE FROM releases WHERE {PAST_WINDOW} RETURNING name"),
        cutoff,
    )
    .await
    .map_err(purge_failure)?;
    let pages = sorted_names(
        &mut *tx,
        &format!("DELETE FROM pages WHERE {PAST_WINDOW} RETURNING name"),
        cutoff,
    )
    .await
    .map_err(purge_failure)?;

    // Artists last; the rows that blocked them are gone by now, and
    // `skipped_artists` names the ones live rows still pin.
    let artists = sorted_names(
        &mut *tx,
        &format!("DELETE FROM artists WHERE {PAST_WINDOW} AND NOT {ARTIST_REFS} RETURNING name"),
        cutoff,
    )
    .await
    .map_err(purge_failure)?;
    let skipped_artists = sorted_names(
        &mut *tx,
        &format!("SELECT name FROM artists WHERE {PAST_WINDOW} AND {ARTIST_REFS}"),
        cutoff,
    )
    .await
    .map_err(purge_failure)?;

    tx.commit().await.map_err(purge_failure)?;

    // The rows are gone for good; the orphaned files follow them off
    // disk. Doing it after the commit keeps the two halves in the same
    // order a reader sees: no file vanishes while a row still names it.
    let files = upload_root.map_or_else(Vec::new, |root| remove_upload_files(root, &orphans));

    Ok(PurgeResult {
        tracks,
        releases,
        artists,
        pages,
        skipped_artists,
        files,
    })
}

/// The read-only twin of the purge: the same fragments, as SELECTs.
#[cfg(feature = "server")]
async fn preview(
    pool: &PgPool,
    cutoff: chrono::DateTime<Utc>,
    upload_root: Option<&std::path::Path>,
) -> sqlx::Result<PurgeResult> {
    let orphans = match upload_root {
        Some(root) => merge_orphans(
            orphan_uploads(pool, cutoff).await?,
            unreferenced_uploads(pool, root).await?,
        ),
        None => Vec::new(),
    };
    Ok(PurgeResult {
        tracks: sorted_names(
            pool,
            &format!("SELECT name FROM tracks WHERE {DOOMED_TRACKS}"),
            cutoff,
        )
        .await?,
        releases: sorted_names(
            pool,
            &format!("SELECT name FROM releases WHERE {PAST_WINDOW}"),
            cutoff,
        )
        .await?,
        artists: sorted_names(
            pool,
            &format!("SELECT name FROM artists WHERE {PAST_WINDOW} AND NOT {ARTIST_REFS}"),
            cutoff,
        )
        .await?,
        pages: sorted_names(
            pool,
            &format!("SELECT name FROM pages WHERE {PAST_WINDOW}"),
            cutoff,
        )
        .await?,
        skipped_artists: sorted_names(
            pool,
            &format!("SELECT name FROM artists WHERE {PAST_WINDOW} AND {ARTIST_REFS}"),
            cutoff,
        )
        .await?,
        files: orphans
            .iter()
            .map(|(dir, file)| format!("{dir}/{file}"))
            .collect(),
    })
}

/// Collect one column of names and sort them, so both surfaces list the
/// same rows in the same order. Works against a pool (previews) or a
/// transaction (the purge's `DELETE ... RETURNING name`).
async fn sorted_names<'e, E>(
    exec: E,
    sql: &str,
    cutoff: chrono::DateTime<Utc>,
) -> sqlx::Result<Vec<String>>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    // Audited for sqlx's injection guard: every caller hands a query built
    // from const `&'static str` fragments (the `PAST_WINDOW`/`ARTIST_REFS`
    // pieces); the only value is the bound `cutoff`.
    let mut names = sqlx::query_scalar::<_, String>(sqlx::AssertSqlSafe(sql))
        .bind(cutoff)
        .fetch_all(exec)
        .await?;
    names.sort();
    Ok(names)
}

/// The `<dir, file>` pairs the purge orphans, sorted, each one a plain
/// file name in a known uploads subdirectory.
///
/// A hand-edited row could name anything, so a name with a path
/// separator, a traversal, or an unknown directory is logged and dropped:
/// neither the preview nor the removal will ever touch the file store
/// with it.
async fn orphan_uploads<'e, E>(
    exec: E,
    cutoff: chrono::DateTime<Utc>,
) -> sqlx::Result<Vec<(String, String)>>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    // Audited for sqlx's injection guard: `orphan_uploads_sql` is assembled
    // from const `&'static str` fragments; the only value is the bound
    // `cutoff`.
    let rows = sqlx::query(sqlx::AssertSqlSafe(orphan_uploads_sql()))
        .bind(cutoff)
        .fetch_all(exec)
        .await?;
    let mut orphans = Vec::new();
    for row in rows {
        let dir: String = row.try_get("dir")?;
        let file: String = row.try_get("file")?;
        if safe_upload_path(&dir, &file).is_some() {
            orphans.push((dir, file));
        } else {
            tracing::warn!("purge ignoring upload reference with an unsafe name: {dir}/{file}");
        }
    }
    orphans.sort();
    Ok(orphans)
}

/// The uploads subdirectories the sweep walks, and the SQL that names the
/// files still referenced in each: an entry of the directory missing from
/// that set is a file no row names anymore.
///
/// Soft-deleted (restorable) rows count as references: only a file no row
/// at all names is fair game, so restoring a row before its purge still
/// finds its file on disk.
const SWEEPS: [(&str, &str); 4] = [
    (
        "artists",
        "SELECT primary_image FROM artists WHERE primary_image IS NOT NULL",
    ),
    (
        "releases",
        "SELECT primary_image FROM releases WHERE primary_image IS NOT NULL",
    ),
    (
        "tracks",
        "SELECT primary_image FROM tracks WHERE primary_image IS NOT NULL
         UNION SELECT audio FROM tracks WHERE audio IS NOT NULL",
    ),
    (
        "avatars",
        "SELECT avatar FROM users WHERE avatar IS NOT NULL",
    ),
];

/// Files sitting in the uploads directories that no row names: replaced
/// uploads, and files their rows lost to purges predating this service.
///
/// The directory is listed *before* the references are queried, so a
/// save that lands in between still counts as a reference (and a fresh
/// upload always names itself a new timestamped file, which the earlier
/// listing cannot have seen unreferenced).
async fn unreferenced_uploads(
    pool: &PgPool,
    root: &std::path::Path,
) -> sqlx::Result<Vec<(String, String)>> {
    let mut orphans = Vec::new();
    for (dir, referenced_sql) in SWEEPS {
        // A missing or unreadable directory offers nothing to remove:
        // never sweep on a listing you could not read.
        let listed: Vec<String> = match std::fs::read_dir(root.join(dir)) {
            Ok(entries) => entries
                .filter_map(|entry| {
                    let entry = entry.ok()?;
                    // Only plain files, top level: subdirectories
                    // (however they got there) and anything not a
                    // regular file are left alone.
                    entry
                        .file_type()
                        .ok()?
                        .is_file()
                        .then(|| entry.file_name().into_string().ok())?
                })
                .collect(),
            Err(problem) if problem.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(problem) => {
                tracing::warn!("purge sweep could not list {dir}: {problem}");
                Vec::new()
            }
        };
        let referenced: std::collections::HashSet<String> = sqlx::query_scalar(referenced_sql)
            .fetch_all(pool)
            .await?
            .into_iter()
            .collect();
        for file in listed {
            if !referenced.contains(&file) && safe_upload_path(dir, &file).is_some() {
                orphans.push((dir.to_string(), file));
            }
        }
    }
    orphans.sort();
    Ok(orphans)
}

/// The row-derived and on-disk orphan sets, merged for the removal pass.
///
/// They are disjoint by construction (the first names files a doomed row
/// names, the second files no row names), but the sort+dedup keeps the
/// listing and the removal order one list anyway.
fn merge_orphans(
    mut row_orphans: Vec<(String, String)>,
    disk_orphans: Vec<(String, String)>,
) -> Vec<(String, String)> {
    row_orphans.extend(disk_orphans);
    row_orphans.sort();
    row_orphans.dedup();
    row_orphans
}

/// A `<dir>/<file>` pair as a path under the uploads root, when `dir` is a
/// known uploads subdirectory and `file` a plain file name.
fn safe_upload_path(dir: &str, file: &str) -> Option<std::path::PathBuf> {
    if !UPLOAD_DIRS.contains(&dir)
        || file.is_empty()
        || file == "."
        || file == ".."
        || file.contains('/')
        || file.contains('\\')
        || file.contains('\0')
    {
        return None;
    }
    Some(std::path::Path::new(dir).join(file))
}

/// Remove the orphaned files and answer with the `<dir>/<file>` names gone
/// from disk, in order.
///
/// A file already missing counts as removed (a previous run may have gone
/// halfway, and the promised state is already true); a file the disk
/// refuses to give up is logged and left out of the list — it is not worth
/// failing a purge that has already committed.
fn remove_upload_files(root: &std::path::Path, orphans: &[(String, String)]) -> Vec<String> {
    let mut removed = Vec::new();
    for (dir, file) in orphans {
        // `safe_upload_path` already vetted the pair at query time.
        let Some(path) = safe_upload_path(dir, file) else {
            continue;
        };
        match std::fs::remove_file(root.join(path)) {
            Ok(()) => removed.push(format!("{dir}/{file}")),
            Err(problem) if problem.kind() == std::io::ErrorKind::NotFound => {
                removed.push(format!("{dir}/{file}"));
            }
            Err(problem) => tracing::warn!("purge could not remove {dir}/{file}: {problem}"),
        }
    }
    removed
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

    /// Write an upload file where uploads live (`<root>/<dir>/<file>`).
    fn put_upload(root: &std::path::Path, dir: &str, file: &str) {
        let subdirectory = root.join(dir);
        std::fs::create_dir_all(&subdirectory).expect("uploads subdirectory");
        std::fs::write(subdirectory.join(file), b"content").expect("upload file");
    }

    /// Point a track's image/audio columns at stored names, as an upload
    /// tail would (the test helpers create rows without uploads).
    async fn set_track_uploads(
        pool: &PgPool,
        track: &crate::models::track::Track,
        audio: &str,
        image: &str,
    ) {
        sqlx::query("UPDATE tracks SET audio = $1, primary_image = $2 WHERE id = $3")
            .bind(audio)
            .bind(image)
            .bind(track.id)
            .execute(pool)
            .await
            .expect("track uploads attached");
    }

    #[sqlx::test]
    async fn purge_requires_admin(pool: PgPool) {
        let permissions = vec!["label_owner"];
        let owner = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .expect("label_owner user");

        let result = purge_deleted_service(&pool, Some(&owner), true, None).await;
        assert!(result.is_err(), "label_owner alone may not purge");

        let guest = purge_deleted_service(&pool, None, true, None).await;
        assert!(guest.is_err(), "guests may not purge");

        // Even a preview is admin-only: the names it lists are admin data.
        let preview = purge_deleted_service(&pool, Some(&owner), false, None).await;
        assert!(preview.is_err(), "label_owner alone may not preview");
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

        let preview = purge_deleted_service(&pool, Some(&admin), false, None)
            .await
            .expect("preview runs");
        assert_eq!(preview.artists, ["Test Artist 1"]);
        assert_eq!(preview.skipped_artists, Vec::<String>::new());

        // The read-only preview left the row where it was, restorable as
        // before.
        let still_there = crate::models::artist::Artist::get_by_id(&pool, artist.id)
            .await
            .expect("fetch artist");
        assert!(still_there.deleted_at.is_some());

        // A confirm on the same state now really removes it.
        let purged = purge_deleted_service(&pool, Some(&admin), true, None)
            .await
            .expect("purge runs");
        assert_eq!(purged.artists, ["Test Artist 1"]);
        let gone = crate::models::artist::Artist::find_by_id(&pool, artist.id)
            .await
            .expect("fetch artist");
        assert!(gone.is_none(), "the confirmed purge removed the row");
    }

    #[sqlx::test]
    async fn preview_names_the_rows_the_purge_removes(pool: PgPool) {
        let admin = create_test_user_with_permissions(&pool, 1, vec!["admin"])
            .await
            .expect("admin user");
        let label = create_test_record_label(&pool, 1).await.expect("label");
        // Artist and its only release are both past the window: the
        // release goes first, which frees the artist.
        let artist = create_test_artist(&pool, 1, Some(label.clone()))
            .await
            .expect("artist");
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .expect("release");
        age_release(&pool, release).await;
        age_artist(&pool, artist.clone()).await;
        let page = create_test_page(&pool, 1, Some(label.clone()))
            .await
            .expect("page");
        age_page(&pool, page.clone()).await;

        let preview = purge_deleted_service(&pool, Some(&admin), false, None)
            .await
            .expect("preview runs");
        assert_eq!(preview.artists, ["Test Artist 1"]);
        assert_eq!(preview.releases, ["Test Release 1"]);
        assert_eq!(preview.pages, ["Test Page 1"]);

        // The confirmed purge answers with exactly the names the preview
        // showed: same fragments, same rules.
        let purged = purge_deleted_service(&pool, Some(&admin), true, None)
            .await
            .expect("purge runs");
        assert_eq!(preview, purged);
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

        let result = purge_deleted_service(&pool, Some(&admin), true, None)
            .await
            .expect("purge runs");

        // Two cascaded tracks (one live!) and one old soft-deleted track.
        assert_eq!(
            result.tracks,
            ["Test Track 1", "Test Track 2", "Test Track 3"]
        );
        assert_eq!(result.releases, ["Test Release 1"]);
        assert_eq!(result.pages, ["Test Page 1"]);

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
        assert_eq!(result.artists, Vec::<String>::new());
        assert_eq!(result.skipped_artists, ["Test Artist 1"]);
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

        let result = purge_deleted_service(&pool, Some(&admin), true, None)
            .await
            .expect("purge runs");
        assert_eq!(result.releases, ["Test Release 1"]);
        assert_eq!(result.artists, ["Test Artist 1"]);
        assert_eq!(result.skipped_artists, Vec::<String>::new());
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

        let result = purge_deleted_service(&pool, Some(&admin), true, None)
            .await
            .expect("purge runs");
        assert_eq!(result.total(), 0);
        assert_eq!(result.skipped_artists, Vec::<String>::new());

        let artist = crate::models::artist::Artist::get_by_id(&pool, artist.id)
            .await
            .expect("fetch artist");
        assert!(artist.deleted_at.is_some());
    }

    // --- orphaned uploads -------------------------------------------------

    #[sqlx::test]
    async fn preview_lists_orphan_uploads_then_the_purge_removes_them(pool: PgPool) {
        let admin = create_test_user_with_permissions(&pool, 1, vec!["admin"])
            .await
            .expect("admin user");
        let label = create_test_record_label(&pool, 1).await.expect("label");
        let artist = create_test_artist(&pool, 1, Some(label.clone()))
            .await
            .expect("artist");
        sqlx::query("UPDATE artists SET primary_image = $1 WHERE id = $2")
            .bind("old-artist.jpg")
            .bind(artist.id)
            .execute(&pool)
            .await
            .expect("artist image attached");

        // A past-window release and one of its (never-deleted) tracks:
        // the track cascades, and both of its uploads go with it.
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .expect("release");
        sqlx::query("UPDATE releases SET primary_image = $1 WHERE id = $2")
            .bind("old-cover.png")
            .bind(release.id)
            .execute(&pool)
            .await
            .expect("release image attached");
        let cascaded = create_test_track(&pool, 1, Some(release.clone()), Some(artist.clone()))
            .await
            .expect("cascaded track");
        set_track_uploads(&pool, &cascaded, "cascaded.flac", "cascaded.png").await;

        age_release(&pool, release).await;
        age_artist(&pool, artist).await;

        let uploads = tempfile::tempdir().expect("uploads dir");
        put_upload(uploads.path(), "artists", "old-artist.jpg");
        put_upload(uploads.path(), "releases", "old-cover.png");
        put_upload(uploads.path(), "tracks", "cascaded.flac");
        put_upload(uploads.path(), "tracks", "cascaded.png");

        let want = [
            "artists/old-artist.jpg",
            "releases/old-cover.png",
            "tracks/cascaded.flac",
            "tracks/cascaded.png",
        ];
        let preview = purge_deleted_service(&pool, Some(&admin), false, Some(uploads.path()))
            .await
            .expect("preview runs");
        assert_eq!(preview.files, want);
        // The preview promised the removals; it did not perform them.
        for label in want {
            assert!(
                uploads.path().join(label).is_file(),
                "the preview left {label} on disk"
            );
        }

        let purged = purge_deleted_service(&pool, Some(&admin), true, Some(uploads.path()))
            .await
            .expect("purge runs");
        assert_eq!(purged.files, want);
        for label in want {
            assert!(
                !uploads.path().join(label).exists(),
                "the purge removed {label} from disk"
            );
        }
    }

    #[sqlx::test]
    async fn uploads_named_by_surviving_rows_are_left_alone(pool: PgPool) {
        let admin = create_test_user_with_permissions(&pool, 1, vec!["admin"])
            .await
            .expect("admin user");
        let label = create_test_record_label(&pool, 1).await.expect("label");
        let artist = create_test_artist(&pool, 1, Some(label.clone()))
            .await
            .expect("artist");

        // A doomed track and a live track share one stored audio name: the
        // survivor still names it, so the file stays and is never offered.
        let keeper_release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .expect("keeper release");
        let doomed_track =
            create_test_track(&pool, 1, Some(keeper_release.clone()), Some(artist.clone()))
                .await
                .expect("doomed track");
        set_track_uploads(&pool, &doomed_track, "shared.flac", "gone.png").await;
        age_track(&pool, doomed_track).await;
        let live_track =
            create_test_track(&pool, 2, Some(keeper_release.clone()), Some(artist.clone()))
                .await
                .expect("live track");
        set_track_uploads(&pool, &live_track, "shared.flac", "live.png").await;

        // A past-window artist blocked from removal keeps its own image:
        // it stays in the database, so its file is a kept reference.
        sqlx::query("UPDATE artists SET primary_image = $1 WHERE id = $2")
            .bind("kept-artist.jpg")
            .bind(artist.id)
            .execute(&pool)
            .await
            .expect("artist image attached");
        age_artist(&pool, artist).await;

        let uploads = tempfile::tempdir().expect("uploads dir");
        put_upload(uploads.path(), "tracks", "shared.flac");
        put_upload(uploads.path(), "tracks", "gone.png");
        put_upload(uploads.path(), "tracks", "live.png");
        put_upload(uploads.path(), "artists", "kept-artist.jpg");

        // Only the name nothing survives on is offered: the doomed track's
        // other upload.
        let preview = purge_deleted_service(&pool, Some(&admin), false, Some(uploads.path()))
            .await
            .expect("preview runs");
        assert_eq!(preview.files, ["tracks/gone.png"]);

        let purged = purge_deleted_service(&pool, Some(&admin), true, Some(uploads.path()))
            .await
            .expect("purge runs");
        assert_eq!(purged.files, ["tracks/gone.png"]);
        assert!(!uploads.path().join("tracks/gone.png").exists());
        for keep in [
            "tracks/shared.flac",
            "tracks/live.png",
            "artists/kept-artist.jpg",
        ] {
            assert!(
                uploads.path().join(keep).is_file(),
                "the purge left {keep} alone"
            );
        }
    }

    #[sqlx::test]
    async fn a_shared_name_two_doomed_rows_have_is_removed_once(pool: PgPool) {
        let admin = create_test_user_with_permissions(&pool, 1, vec!["admin"])
            .await
            .expect("admin user");
        let label = create_test_record_label(&pool, 1).await.expect("label");
        let artist = create_test_artist(&pool, 1, Some(label.clone()))
            .await
            .expect("artist");
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .expect("release");
        for n in 1..=2 {
            let track = create_test_track(&pool, n, Some(release.clone()), Some(artist.clone()))
                .await
                .expect("track");
            set_track_uploads(&pool, &track, "same.flac", "cover.png").await;
        }
        age_release(&pool, release).await;

        let uploads = tempfile::tempdir().expect("uploads dir");
        put_upload(uploads.path(), "tracks", "same.flac");
        put_upload(uploads.path(), "tracks", "cover.png");

        // Two doomed rows, one file each pair: the lists fold to one name
        // and the second removal quietly counts as done (already gone).
        let preview = purge_deleted_service(&pool, Some(&admin), false, Some(uploads.path()))
            .await
            .expect("preview runs");
        assert_eq!(preview.files, ["tracks/cover.png", "tracks/same.flac"]);
        let purged = purge_deleted_service(&pool, Some(&admin), true, Some(uploads.path()))
            .await
            .expect("purge runs");
        assert_eq!(purged.files, ["tracks/cover.png", "tracks/same.flac"]);
        assert_eq!(
            std::fs::read_dir(uploads.path().join("tracks"))
                .expect("tracks dir")
                .count(),
            0
        );
    }

    #[sqlx::test]
    async fn unsafe_upload_names_are_never_offered_nor_removed(pool: PgPool) {
        let admin = create_test_user_with_permissions(&pool, 1, vec!["admin"])
            .await
            .expect("admin user");
        let label = create_test_record_label(&pool, 1).await.expect("label");
        let artist = create_test_artist(&pool, 1, Some(label.clone()))
            .await
            .expect("artist");
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .expect("release");
        let doomed_track = create_test_track(&pool, 1, Some(release.clone()), Some(artist.clone()))
            .await
            .expect("doomed track");
        // A hand-edited row reaching outside the uploads subdirectory.
        set_track_uploads(&pool, &doomed_track, "../escape.png", "dodgy\\name.png").await;
        age_release(&pool, release).await;

        let uploads = tempfile::tempdir().expect("uploads dir");
        std::fs::write(uploads.path().join("escape.png"), b"do not touch").expect("escape file");

        let preview = purge_deleted_service(&pool, Some(&admin), false, Some(uploads.path()))
            .await
            .expect("preview runs");
        assert_eq!(
            preview.files,
            Vec::<String>::new(),
            "no unsafe name is offered"
        );

        let purged = purge_deleted_service(&pool, Some(&admin), true, Some(uploads.path()))
            .await
            .expect("purge runs");
        assert_eq!(purged.files, Vec::<String>::new());
        assert_eq!(purged.tracks, ["Test Track 1"], "the row still goes");
        assert!(
            uploads.path().join("escape.png").is_file(),
            "the file outside the upload dirs was never touched"
        );
    }

    #[sqlx::test]
    async fn a_file_already_missing_still_counts_as_removed(pool: PgPool) {
        let admin = create_test_user_with_permissions(&pool, 1, vec!["admin"])
            .await
            .expect("admin user");
        let label = create_test_record_label(&pool, 1).await.expect("label");
        let artist = create_test_artist(&pool, 1, Some(label.clone()))
            .await
            .expect("artist");
        let doomed_track = create_test_track(&pool, 1, None, Some(artist.clone()))
            .await
            .expect("doomed track");
        set_track_uploads(&pool, &doomed_track, "ghost.flac", "on-disk.png").await;
        age_track(&pool, doomed_track).await;

        let uploads = tempfile::tempdir().expect("uploads dir");
        put_upload(uploads.path(), "tracks", "on-disk.png");
        // "ghost.flac" is named by the row but was never written (or a
        // previous run got halfway): both surfaces list it as removed.

        let preview = purge_deleted_service(&pool, Some(&admin), false, Some(uploads.path()))
            .await
            .expect("preview runs");
        assert_eq!(preview.files, ["tracks/ghost.flac", "tracks/on-disk.png"]);
        let purged = purge_deleted_service(&pool, Some(&admin), true, Some(uploads.path()))
            .await
            .expect("purge runs");
        assert_eq!(purged.files, ["tracks/ghost.flac", "tracks/on-disk.png"]);
    }

    #[sqlx::test]
    async fn without_an_uploads_root_files_are_never_offered(pool: PgPool) {
        let admin = create_test_user_with_permissions(&pool, 1, vec!["admin"])
            .await
            .expect("admin user");
        let label = create_test_record_label(&pool, 1).await.expect("label");
        let artist = create_test_artist(&pool, 1, Some(label.clone()))
            .await
            .expect("artist");
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .expect("release");
        let track = create_test_track(&pool, 1, Some(release.clone()), Some(artist.clone()))
            .await
            .expect("track");
        set_track_uploads(&pool, &track, "song.flac", "cover.png").await;
        age_release(&pool, release).await;

        // The rows still go, but no file lists are offered (the preview
        // promises nothing the server could not deliver).
        let preview = purge_deleted_service(&pool, Some(&admin), false, None)
            .await
            .expect("preview runs");
        assert_eq!(preview.tracks, ["Test Track 1"]);
        assert_eq!(preview.files, Vec::<String>::new());
        let purged = purge_deleted_service(&pool, Some(&admin), true, None)
            .await
            .expect("purge runs");
        assert_eq!(purged.files, Vec::<String>::new());
        assert!(
            crate::models::track::Track::find_by_slug(&pool, "test-track-1".to_string())
                .await
                .expect("fetch track")
                .is_none()
        );
    }

    /// Point an artist's image column at a stored name, as an upload tail
    /// would.
    async fn set_artist_image(pool: &PgPool, artist: &crate::models::artist::Artist, image: &str) {
        sqlx::query("UPDATE artists SET primary_image = $1 WHERE id = $2")
            .bind(image)
            .bind(artist.id)
            .execute(pool)
            .await
            .expect("artist image attached");
    }

    #[sqlx::test]
    async fn files_named_by_no_row_are_listed_then_swept(pool: PgPool) {
        let admin = create_test_user_with_permissions(&pool, 1, vec!["admin"])
            .await
            .expect("admin user");

        let uploads = tempfile::tempdir().expect("uploads dir");
        put_upload(uploads.path(), "artists", "old-artist.png");
        put_upload(uploads.path(), "avatars", "old-avatar.png");

        // Nothing is past any window: the sweep alone is what makes a
        // file-only purge worth running.
        let preview = purge_deleted_service(&pool, Some(&admin), false, Some(uploads.path()))
            .await
            .expect("preview runs");
        assert_eq!(preview.total(), 0, "no rows are doomed");
        assert_eq!(
            preview.files,
            ["artists/old-artist.png", "avatars/old-avatar.png"]
        );

        let purged = purge_deleted_service(&pool, Some(&admin), true, Some(uploads.path()))
            .await
            .expect("purge runs");
        assert_eq!(
            purged.files,
            ["artists/old-artist.png", "avatars/old-avatar.png"]
        );
        for gone in ["artists/old-artist.png", "avatars/old-avatar.png"] {
            assert!(
                !uploads.path().join(gone).exists(),
                "{gone} should be off disk"
            );
        }
    }

    #[sqlx::test]
    async fn a_replaced_upload_is_swept_and_the_current_file_stays(pool: PgPool) {
        let admin = create_test_user_with_permissions(&pool, 1, vec!["admin"])
            .await
            .expect("admin user");
        let label = create_test_record_label(&pool, 1).await.expect("label");
        let artist = create_test_artist(&pool, 1, Some(label))
            .await
            .expect("artist");
        set_artist_image(&pool, &artist, "current.png").await;

        let uploads = tempfile::tempdir().expect("uploads dir");
        put_upload(uploads.path(), "artists", "current.png");
        // What the artist named before the replacement uploaded left
        // behind: no row names it, so the sweep takes it.
        put_upload(uploads.path(), "artists", "previous.png");

        let preview = purge_deleted_service(&pool, Some(&admin), false, Some(uploads.path()))
            .await
            .expect("preview runs");
        assert_eq!(preview.files, ["artists/previous.png"]);
        let purged = purge_deleted_service(&pool, Some(&admin), true, Some(uploads.path()))
            .await
            .expect("purge runs");
        assert_eq!(purged.files, ["artists/previous.png"]);
        assert!(
            uploads.path().join("artists/current.png").is_file(),
            "the file the row still names must survive the sweep"
        );
    }

    #[sqlx::test]
    async fn a_restorable_row_keeps_its_file(pool: PgPool) {
        // A soft-deleted row *inside* its window can still be restored,
        // so it counts as a reference: its file is not an orphan yet.
        let admin = create_test_user_with_permissions(&pool, 1, vec!["admin"])
            .await
            .expect("admin user");
        let label = create_test_record_label(&pool, 1).await.expect("label");
        let artist = create_test_artist(&pool, 1, Some(label))
            .await
            .expect("artist");
        set_artist_image(&pool, &artist, "waiting.png").await;
        // Soft-deleted recently, so still inside the window: restorable,
        // therefore still a reference. Nothing to keep hold of afterwards.
        artist.delete(&pool).await.expect("soft delete artist");

        let uploads = tempfile::tempdir().expect("uploads dir");
        put_upload(uploads.path(), "artists", "waiting.png");

        let preview = purge_deleted_service(&pool, Some(&admin), false, Some(uploads.path()))
            .await
            .expect("preview runs");
        assert_eq!(preview.files, Vec::<String>::new());
        assert!(uploads.path().join("artists/waiting.png").is_file());
    }

    #[sqlx::test]
    async fn staging_and_stray_subdirectories_are_never_swept(pool: PgPool) {
        let admin = create_test_user_with_permissions(&pool, 1, vec!["admin"])
            .await
            .expect("admin user");

        let uploads = tempfile::tempdir().expect("uploads dir");
        put_upload(uploads.path(), "tmp", "upload-in-progress.wav");
        let nested = uploads.path().join("tracks").join("nested");
        std::fs::create_dir_all(&nested).expect("nested dir");
        std::fs::write(nested.join("who-knows-why.png"), b"content").expect("nested file");

        let purged = purge_deleted_service(&pool, Some(&admin), true, Some(uploads.path()))
            .await
            .expect("purge runs");
        assert_eq!(purged.files, Vec::<String>::new());
        assert!(
            uploads.path().join("tmp/upload-in-progress.wav").is_file(),
            "staging is never purge's business"
        );
        assert!(
            nested.join("who-knows-why.png").is_file(),
            "the sweep only ever takes plain files at the top of a known directory"
        );
    }
}
