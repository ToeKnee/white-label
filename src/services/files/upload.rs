//! The shared tail of an upload: once the bytes are in `<root>/tmp`, move
//! them into the config's directory and attach the file to its row.
//!
//! Both upload paths (the dioxus server fn in `ui::upload` and the no-JS
//! form handler in `server::upload_form`) stream the request body their own
//! way, then hand the finished file here, so storage layout and database
//! attachment cannot drift between them.

use sqlx::PgPool;

use crate::config::upload::UploadConfiguration;
use crate::models::artist::Artist;
use crate::models::auth::User;
use crate::models::release::Release;
use crate::services::files::progress::FILES;

/// One completed upload: where it landed in `tmp/` and its two names.
pub struct UploadedFile {
    /// The temp file the streamer wrote (`<root>/tmp/<file_name>`).
    pub tmp_path: String,
    /// The validated storage name from [`crate::utils::files::valid_file_name`].
    pub file_name: String,
    /// The client's own filename, which keys the progress registry.
    pub original_file_name: String,
}

/// Move the finished temp file into the config's directory, drop its
/// progress entry and attach it to the artist/avatar/release row.
///
/// A database-attach error is deliberately swallowed here: the bytes are
/// already stored, so a failed attach only leaves the row pointing at the
/// old file.
///
/// `upload_root` is the uploads root as a path string (`UploadRoot::root`),
/// and `slug` is the item the upload names: an artist or release slug, or
/// the username for an avatar.
///
/// Returns the stored file name.
///
/// # Errors
/// Text describing why the rename failed (the caller turns it into its own
/// error shape).
pub async fn finalise_upload(
    pool: &PgPool,
    upload_root: &str,
    upload_config: UploadConfiguration,
    file: UploadedFile,
    uploader: &User,
    slug: &str,
) -> Result<String, String> {
    let details = upload_config.get_details();
    let path = format!(
        "{upload_root}/{}/{file_name}",
        details.path,
        file_name = file.file_name
    );
    std::fs::rename(file.tmp_path, &path).map_err(|error| error.to_string())?;

    tracing::info!("File uploaded.");
    FILES.remove(&format!(
        "{}-{}",
        uploader.username, file.original_file_name
    ));

    // A failed database attach does not fail the upload: the file itself
    // is already stored.
    let file_name = file.file_name.as_str();
    let attach = match upload_config {
        UploadConfiguration::Artist => store_artist_primary_image(pool, file_name, slug).await,
        UploadConfiguration::Avatar => store_avatar(pool, file_name, slug, uploader).await,
        UploadConfiguration::Release => store_release_primary_image(pool, file_name, slug).await,
    };
    if let Err(error) = attach {
        tracing::error!("uploaded file not attached to {upload_config}: {error}");
    }
    Ok(file.file_name)
}

/// Point an artist's `primary_image` at the uploaded file.
///
/// Only the image column is written. Saving the whole `Artist` instead
/// would rename the row: [`Artist::update`] re-derives `slug` from `name`,
/// so any artist whose slug does not match its name (a hand-edited slug, or
/// a fixture like `e2e-tabs`) silently loses its slug — breaking every link
/// to that artist.
async fn store_artist_primary_image(
    pool: &PgPool,
    file_name: &str,
    slug: &str,
) -> Result<(), String> {
    let artist = Artist::get_by_slug(pool, slug.to_string())
        .await
        .map_err(|error| error.to_string())?;

    sqlx::query("UPDATE artists SET primary_image = $1, updated_at = NOW() WHERE id = $2")
        .bind(file_name)
        .bind(artist.id)
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Point the logged-in user's own avatar at the uploaded file. A slug
/// that doesn't match the uploader's username is a silent no-op.
/// Writes only the `avatar` column, for the same reason as
/// [`store_artist_primary_image`]: the session's copy of the row predates any
/// profile edits made since sign-in, and a whole-row update would roll them
/// back. After an avatar change other pages pick the new file up on their
/// next data fetch; the profile page additionally hands the navbar a
/// fresh `SessionHint::SignedIn` carrying the updated user.
async fn store_avatar(
    pool: &PgPool,
    file_name: &str,
    slug: &str,
    uploader: &User,
) -> Result<(), String> {
    if slug != uploader.username {
        return Ok(());
    }
    sqlx::query("UPDATE users SET avatar = $1, updated_at = NOW() WHERE id = $2")
        .bind(file_name)
        .bind(uploader.id)
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Point a release's `primary_image` at the uploaded file, writing only the
/// image column for the same reason as [`store_artist_primary_image`].
async fn store_release_primary_image(
    pool: &PgPool,
    file_name: &str,
    slug: &str,
) -> Result<(), String> {
    let release = Release::get_by_slug(pool, slug.to_string())
        .await
        .map_err(|error| error.to_string())?;

    sqlx::query("UPDATE releases SET primary_image = $1, updated_at = NOW() WHERE id = $2")
        .bind(file_name)
        .bind(release.id)
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::test_helpers::create_test_user;

    // The contract behind the shared-user race that made the Playwright
    // profile spec flaky: the upload flow must never carry session-time row
    // values back into the database. (The race window itself is too narrow to
    // hit deterministically; the client spec is its regression test.)
    #[sqlx::test]
    fn store_avatar_keeps_the_columns_a_profile_save_owns(pool: PgPool) {
        let sql_user = create_test_user(&pool, 1)
            .await
            .expect("test user to exist");
        let (mut session_copy, _passhash) = sql_user.into_user(None);
        // Stale by design: a copy taken before the rename the DB already has.
        session_copy.first_name = Some("Session-time copy".to_string());

        // A profile save that landed after sign-in but before the upload.
        sqlx::query("UPDATE users SET first_name = $1 WHERE id = $2")
            .bind("Renamed")
            .bind(session_copy.id)
            .execute(&pool)
            .await
            .expect("the profile rename to land");

        store_avatar(&pool, "avatar.png", "test-1", &session_copy)
            .await
            .expect("the avatar to attach");

        let stored = User::get_by_username(&pool, "test-1".to_string())
            .await
            .expect("the stored user");
        assert_eq!(stored.first_name.as_deref(), Some("Renamed"));
        assert_eq!(stored.avatar.as_deref(), Some("avatar.png"));
    }

    #[sqlx::test]
    fn store_avatar_is_a_no_op_for_a_foreign_slug(pool: PgPool) {
        let sql_user = create_test_user(&pool, 2)
            .await
            .expect("test user to exist");
        let (uploader, _passhash) = sql_user.into_user(None);

        store_avatar(&pool, "avatar.png", "someone-else", &uploader)
            .await
            .expect("the slug check to pass quietly");

        let stored = User::get_by_username(&pool, "test-2".to_string())
            .await
            .expect("the stored user");
        assert_eq!(stored.avatar, None);
    }
}
