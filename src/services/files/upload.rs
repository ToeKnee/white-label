//! The shared tail of an upload: once the bytes are in `<root>/tmp`, move
//! them into the config's directory and attach the file to its row.
//!
//! Both serving stacks stream bytes their own way (the Leptos wire protocol
//! in `server::api::files` reads ordered multipart fields; the dioxus server
//! fn in `ui::upload` consumes a request body stream), then hand the
//! finished file here, so storage layout and database attachment cannot
//! drift between them.

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
/// This mirrors the Leptos `finalise_file_upload`, including its swallowing
/// of database-attach errors.
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

    // Database attachment never fails the upload today (the Leptos route
    // ignores these errors) — kept as-is for parity between the stacks.
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
async fn store_artist_primary_image(
    pool: &PgPool,
    file_name: &str,
    slug: &str,
) -> Result<(), String> {
    let mut artist = Artist::get_by_slug(pool, slug.to_string())
        .await
        .map_err(|error| error.to_string())?;
    artist.primary_image = Some(file_name.to_string());
    artist
        .update(pool)
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Point the logged-in user's own avatar at the uploaded file. A slug
/// that doesn't match the uploader's username is a silent no-op, as in the
/// Leptos route. (It also refreshed Leptos' client-side user context —
/// dioxus clients re-fetch instead.)
async fn store_avatar(
    pool: &PgPool,
    file_name: &str,
    slug: &str,
    uploader: &User,
) -> Result<(), String> {
    if slug != uploader.username {
        return Ok(());
    }
    let mut user = User::get_by_username(pool, slug.to_string())
        .await
        .map_err(|error| error.to_string())?;
    user.avatar = Some(file_name.to_string());
    user.update(pool)
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Point a release's `primary_image` at the uploaded file.
async fn store_release_primary_image(
    pool: &PgPool,
    file_name: &str,
    slug: &str,
) -> Result<(), String> {
    let mut release = Release::get_by_slug(pool, slug.to_string())
        .await
        .map_err(|error| error.to_string())?;
    release.primary_image = Some(file_name.to_string());
    release
        .update(pool)
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}
