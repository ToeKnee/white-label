//! The javascript-off upload form.
//!
//! The dioxus upload widget speaks a request-body protocol
//! (`POST /api/upload` with a streamed `FileStream`), which needs the wasm
//! client. Its markup also carries a plain `multipart/form-data` form, so a
//! browser with javascript off can still upload; this module answers that
//! form.
//!
//! The two paths share everything that matters: the same filename validation
//! (`utils::files::valid_file_name`), the same progress registry, and the same
//! [`crate::services::files::upload::finalise_upload`] tail that moves the
//! temp file into place and attaches it to the row.
//!
//! Example (curl, against a signed-in session):
//!
//! ```text
//! POST /forms/upload   multipart/form-data
//!   file_type=Artist  slug=test-artist-1  file_to_upload=<cover.png>
//! 303 -> /admin/artist/test-artist-1/images?uploaded=1
//! ```

use axum::Extension;
use axum::extract::Request;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Redirect, Response};
use futures::TryStreamExt as _;
use std::io::Write as _;

use crate::config::upload::UploadConfiguration;
use crate::error::AppError;
use crate::server::UploadRoot;
use crate::server::current_user::{CurrentUser, Db};
use crate::services::authentication_helpers::user_with_permissions;
use crate::services::files::progress::{FILES, add_chunk};
use crate::services::files::upload::{UploadedFile, finalise_upload};
use crate::utils::files::{
    SIGNATURE_BYTES, audio_mime_in_bytes, claimed_upload_mime, first_bytes, image_mime_in_bytes,
    valid_file_name,
};

/// Where the widget's javascript-off form posts.
pub const PATH_UPLOAD_FORM: &str = "/forms/upload";

/// Answers the widget's javascript-off form post.
///
/// Field order is fixed by the widget's markup: `file_type` (`Artist`,
/// `Avatar`, `Release` or `TrackAudio`), `slug`, then `artist_slug` and
/// `release_slug` (only releases and tracks carry them), then the
/// `file_to_upload` part. The bytes stream to `<root>/tmp/<name>` under the
/// configuration's size limit, their first bytes have to match one of the
/// configuration's image or audio types (the part's own content type is the
/// browser's guess, taken from the file name), and then
/// [`finalise_upload`] moves and attaches them — exactly as the streaming
/// server fn does them.
///
/// Like the other form handlers the answer is a 303 to a literal page:
/// `?uploaded=1` on success, `?bad_upload=<code>` on failure
/// ([`crate::ui::upload::upload_message`] owns the code→text table). A
/// failure before the page can be worked out (no multipart body, unknown
/// configuration, non-canonical slug) comes back to `/admin`.
#[expect(
    clippy::too_many_lines,
    reason = "one straight-line validation pass over a multipart body; splitting a request that must be read in order spreads the borrow of `field` across functions"
)]
pub(super) async fn upload_image(
    Db(pool): Db,
    CurrentUser(session_user): CurrentUser,
    Extension(upload_root): Extension<UploadRoot>,
    request: Request,
) -> Response {
    let Some(session_user) = session_user else {
        return Redirect::to("/login").into_response();
    };

    let Ok(boundary) = multer::parse_boundary(
        request
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default(),
    ) else {
        tracing::warn!("upload form arrived without a multipart boundary");
        return failed_upload("/admin", "bad_form");
    };
    let mut multipart = multer::Multipart::new(
        request
            .into_body()
            .into_data_stream()
            .map_err(|error| multer::Error::StreamReadFailed(Box::new(error))),
        boundary,
    );

    // The header fields precede the file part in the widget's markup, and
    // they decide the configuration, the row and the page to come back to.
    let mut file_type = String::new();
    let mut slug = String::new();
    let mut artist_slug = String::new();
    let mut release_slug = String::new();
    let mut file_part = None;
    while let Some(field) = next_form_field(&mut multipart).await {
        if field.file_name().is_some() {
            file_part = Some(field);
            break;
        }
        let name = field.name().unwrap_or_default().to_string();
        let value = match name.as_str() {
            "file_type" | "slug" | "artist_slug" | "release_slug" => match field.text().await {
                Ok(text) => text,
                Err(error) => {
                    tracing::warn!("upload form field {name} could not be read: {error}");
                    return failed_upload("/admin", "bad_form");
                }
            },
            other => {
                tracing::debug!("ignoring unexpected upload form field {other}");
                String::new()
            }
        };
        match name.as_str() {
            "file_type" => file_type = value,
            "slug" => slug = value,
            "artist_slug" => artist_slug = value,
            "release_slug" => release_slug = value,
            _ => {}
        }
    }

    let Ok(upload_config) = file_type.parse::<UploadConfiguration>() else {
        tracing::warn!("upload form carried an unknown configuration: {file_type}");
        return failed_upload("/admin", "bad_form");
    };
    let Some(page) = upload_target(upload_config, &slug, &artist_slug, &release_slug) else {
        tracing::warn!("upload form carried a slug that is not a slugify result: {slug}");
        return failed_upload("/admin", "bad_form");
    };
    let details = upload_config.get_details();

    let required_permissions = details.permissions.iter().map(String::as_str).collect();
    let uploader = match user_with_permissions(Some(&session_user), required_permissions) {
        Ok(uploader) => uploader,
        Err(error) => {
            tracing::warn!("upload form rejected: {error}");
            return failed_upload(&page, upload_code(&error));
        }
    };

    let Some(mut field) = file_part else {
        return failed_upload(&page, "missing_file");
    };
    // An untouched file input submits a part with an empty name: not an upload.
    let Some(original_file_name) = field.file_name().filter(|name| !name.is_empty()) else {
        return failed_upload(&page, "missing_file");
    };
    let original_file_name = original_file_name.to_string();
    // `Field::content_type` is a parsed `Mime`; the configurations list
    // accepted types as strings, so the comparison is a string one. The
    // browser's claim is its own mime table's answer for the file's name, and
    // that table can hold a spelling a configuration does not list
    // (`audio/x-flac`), so the extension fills in whatever it is not — exactly
    // as the streaming path works out the same claim.
    let declared_type = field
        .content_type()
        .map_or_else(String::new, ToString::to_string);
    let claimed_type =
        claimed_upload_mime(&declared_type, &original_file_name, &details.mime_types);
    if !details
        .mime_types
        .iter()
        .any(|allowed| allowed == claimed_type)
    {
        return failed_upload(&page, "bad_mime");
    }
    let Ok(stored_name) =
        valid_file_name(&original_file_name, Some(slug.clone()), &details.path, true)
    else {
        tracing::warn!("upload form rejected the file name {original_file_name}");
        return failed_upload(&page, "bad_file_name");
    };
    let Some(root) = upload_root.root() else {
        tracing::error!("upload form arrived but no upload path is configured");
        return failed_upload(&page, "server");
    };

    let tmp_path = format!("{root}/tmp/{stored_name}");
    tracing::info!("Uploading {stored_name} (form) to {tmp_path}");
    // Undoing a rejected upload is the same two steps every time: take the
    // partial file away, and let any progress stream watching it end.
    let progress_key = format!("{}-{original_file_name}", uploader.username);
    let undo_upload = || {
        if let Err(error) = std::fs::remove_file(&tmp_path) {
            tracing::error!("could not remove the partial upload {tmp_path}: {error}");
        }
        FILES.remove(progress_key.as_str());
    };
    let streamed = write_multipart_file(
        &mut field,
        &tmp_path,
        details.size_limit,
        &original_file_name,
        &uploader.username,
    )
    .await;
    if let Err(code) = streamed {
        undo_upload();
        return failed_upload(&page, code);
    }

    // The part's content type is the browser's claim, which it takes from the
    // file's name. The first bytes are what actually arrived, and they decide.
    let actual_type = match first_bytes(&tmp_path, SIGNATURE_BYTES) {
        Ok(signature) => {
            image_mime_in_bytes(&signature).or_else(|| audio_mime_in_bytes(&signature))
        }
        Err(error) => {
            tracing::error!("cannot read back {tmp_path}: {error}");
            undo_upload();
            return failed_upload(&page, "server");
        }
    };
    if !actual_type.is_some_and(|actual| details.mime_types.iter().any(|allowed| allowed == actual))
    {
        tracing::warn!(
            "upload form rejected {original_file_name}: declared {claimed_type}, \
             which is not what its bytes are"
        );
        undo_upload();
        return failed_upload(&page, "bad_image");
    }

    match finalise_upload(
        &pool,
        &root,
        upload_config,
        UploadedFile {
            tmp_path,
            file_name: stored_name.clone(),
            original_file_name: original_file_name.clone(),
        },
        &uploader,
        &slug,
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("{page}?uploaded=1")).into_response(),
        Err(error) => {
            tracing::error!("upload {stored_name} could not be finished: {error}");
            failed_upload(&page, "server")
        }
    }
}

/// The next multipart field, or `None` once the body is exhausted or unreadable.
///
/// A body that fails to parse is not an upload, so the caller treats `None` as
/// "the header fields are all there is".
async fn next_form_field<'a>(multipart: &mut multer::Multipart<'a>) -> Option<multer::Field<'a>> {
    match multipart.next_field().await {
        Ok(field) => field,
        Err(error) => {
            tracing::warn!("upload form body could not be read: {error}");
            None
        }
    }
}

/// Writes one multipart part to `tmp_path`, counting bytes for the progress
/// registry and refusing to pass `size_limit`.
///
/// # Errors
/// `"too_large"` once the part passes the limit, `"server"` if the temp file
/// cannot be created or written.
async fn write_multipart_file(
    field: &mut multer::Field<'_>,
    tmp_path: &str,
    size_limit: usize,
    original_file_name: &str,
    username: &str,
) -> Result<(), &'static str> {
    let mut tmp_file = std::fs::File::create(tmp_path).map_err(|error| {
        tracing::error!("cannot create {tmp_path}: {error}");
        "server"
    })?;
    let mut received = 0_usize;
    while let Some(chunk) = field.chunk().await.map_err(|error| {
        tracing::error!("upload form part for {original_file_name} failed: {error}");
        "server"
    })? {
        if received + chunk.len() > size_limit {
            tracing::warn!(
                "upload {original_file_name} passed the {} MB limit",
                size_limit / (1024 * 1024)
            );
            return Err("too_large");
        }
        tmp_file.write_all(&chunk).map_err(|error| {
            tracing::error!("cannot write {tmp_path}: {error}");
            "server"
        })?;
        received += chunk.len();
        add_chunk(original_file_name, chunk.len(), username).await;
    }
    Ok(())
}

/// A failure answer, so every rejection branch reads the same way.
fn failed_upload(page: &str, code: &str) -> Response {
    Redirect::to(&format!("{page}?bad_upload={code}")).into_response()
}

/// The editor page an upload belongs to, or `None` if the submitted slugs
/// cannot go in a `Location` header.
///
/// The targets are the same literals the app's own links build. Nothing else
/// hosts the widget, so an unmapped configuration has no page.
fn upload_target(
    config: UploadConfiguration,
    slug: &str,
    artist_slug: &str,
    release_slug: &str,
) -> Option<String> {
    match config {
        UploadConfiguration::Avatar => Some("/profile".to_string()),
        UploadConfiguration::Artist if canonical_slug(slug) => {
            Some(format!("/admin/artist/{slug}/images"))
        }
        UploadConfiguration::Release if canonical_slug(slug) && canonical_slug(artist_slug) => {
            Some(format!("/admin/artist/{artist_slug}/release/{slug}"))
        }
        // A track editor is addressed by all three slugs, which is why the
        // widget carries the release's as well as the artist's.
        UploadConfiguration::TrackAudio
            if canonical_slug(slug)
                && canonical_slug(artist_slug)
                && canonical_slug(release_slug) =>
        {
            Some(format!(
                "/admin/artist/{artist_slug}/release/{release_slug}/track/{slug}"
            ))
        }
        _ => None,
    }
}

/// Whether `slug` can safely be put in a redirect.
///
/// `utils::slugify` output is lowercase alphanumeric with dashes, which is what
/// the app's routes use; anything else (a hand-crafted body, or a slug `deunicode`
/// left untouched) gets no redirect target rather than a `Location` header the
/// app would not route.
fn canonical_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 255
        && slug
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// The fixed codes for a failed upload; `crate::ui::upload::upload_message`
/// turns them into the on-screen text.
const fn upload_code(error: &AppError) -> &'static str {
    match error {
        AppError::Unauthorized(_) | AppError::Forbidden(_) => "forbidden",
        AppError::NotFound(_) => "not_found",
        AppError::BadRequest(_) => "bad_form",
        AppError::Internal(_) => "server",
    }
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::{PATH_UPLOAD_FORM, canonical_slug, upload_code, upload_target};
    use crate::config::upload::UploadConfiguration;
    use crate::error::AppError;
    use crate::models::artist::Artist;
    use crate::models::test_helpers::{
        create_test_artist, create_test_record_label, create_test_release, create_test_track,
        create_test_user_with_permissions,
    };
    use crate::models::track::Track;
    use crate::server::test_support::{api_server_with_uploads, login_as};

    const BOUNDARY: &str = "white-label-upload-form-test-boundary";

    /// A real png signature followed by filler: the handler checks the
    /// signature and stores the rest as it came, so a test that is not about
    /// image contents can keep the body short.
    const PNG_BODY: &[u8] = b"\x89PNG\r\n\x1a\npng body";
    /// The same trick for an audio upload: a `fLaC` signature and filler.
    const FLAC_BODY: &[u8] = b"fLaC\0\0\0\x18fake flac body";

    /// The exact field order the widget renders: configuration, slug,
    /// artist slug, then the file part.
    fn form_body(fields: &[(&str, &str)], file: Option<(&str, &str, &[u8])>) -> Vec<u8> {
        let mut body = Vec::new();
        for (name, value) in fields {
            body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
            body.extend_from_slice(
                format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
            );
            body.extend_from_slice(value.as_bytes());
            body.extend_from_slice(b"\r\n");
        }
        if let Some((content_type, file_name, bytes)) = file {
            body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
            body.extend_from_slice(
                format!(
                    "Content-Disposition: form-data; name=\"file_to_upload\"; filename=\"{file_name}\"\r\n\
                     Content-Type: {content_type}\r\n\r\n"
                )
                .as_bytes(),
            );
            body.extend_from_slice(bytes);
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());
        body
    }

    fn post_form(server: &axum_test::TestServer, body: &[u8]) -> axum_test::TestRequest {
        server
            .post(PATH_UPLOAD_FORM)
            .content_type(&format!("multipart/form-data; boundary={BOUNDARY}"))
            .bytes(axum::body::Bytes::copy_from_slice(body))
    }

    /// Where a 303 tells the browser to go next.
    fn location(response: &axum_test::TestResponse) -> String {
        response
            .headers()
            .get(axum::http::header::LOCATION)
            .expect("a 303 should carry a Location")
            .to_str()
            .expect("the Location should be ascii")
            .to_string()
    }

    /// A signed-in `admin` + `label_owner` server plus the `test-artist-1`
    /// fixture and an uploads root with the directories the tail expects.
    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; every caller is a `#[sqlx::test]` body running via block_on on one thread"
    )]
    async fn signed_in_server(
        pool: &PgPool,
        upload_root: &std::path::Path,
    ) -> axum_test::TestServer {
        let label = create_test_record_label(pool, 1).await.expect("label");
        create_test_artist(pool, 1, Some(label))
            .await
            .expect("artist");
        create_test_user_with_permissions(pool, 1, vec!["admin", "label_owner"])
            .await
            .expect("admin + label_owner user");
        for dir in ["tmp", "artists"] {
            std::fs::create_dir_all(upload_root.join(dir)).expect("uploads subdirectory");
        }
        let server = api_server_with_uploads(pool, upload_root.to_path_buf()).await;
        login_as(&server, "test-1").await;
        server
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn upload_form_stores_the_file_and_points_at_the_editor(pool: PgPool) {
        let upload_dir = tempfile::tempdir().expect("upload dir");
        let server = signed_in_server(&pool, upload_dir.path()).await;

        let response = post_form(
            &server,
            &form_body(
                &[("file_type", "Artist"), ("slug", "test-artist-1")],
                Some(("image/png", "cover.png", PNG_BODY)),
            ),
        )
        .await;

        response.assert_status(axum::http::StatusCode::SEE_OTHER);
        assert_eq!(
            location(&response),
            "/admin/artist/test-artist-1/images?uploaded=1"
        );

        // The file moved out of tmp/ and the artist row names it, so the
        // redirect target really does show the new picture.
        let artists_dir = upload_dir.path().join("artists");
        let stored: Vec<std::path::PathBuf> = std::fs::read_dir(&artists_dir)
            .expect("artists dir readable")
            .map(|entry| entry.expect("dir entry").path())
            .collect();
        assert_eq!(stored.len(), 1, "one file should be in artists/");
        let stored_name = stored[0]
            .file_name()
            .expect("file name")
            .to_string_lossy()
            .into_owned();
        assert!(
            stored_name.ends_with("-test-artist-1.png"),
            "stored file should be <timestamp>-test-artist-1.png, got {stored_name}"
        );
        assert_eq!(
            std::fs::read(artists_dir.join(&stored_name)).expect("stored bytes"),
            PNG_BODY
        );
        let artist = Artist::get_by_slug(&pool, "test-artist-1".to_string())
            .await
            .expect("artist after upload");
        assert_eq!(artist.primary_image.as_deref(), Some(stored_name.as_str()));
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn upload_form_without_a_session_goes_to_login(pool: PgPool) {
        let upload_dir = tempfile::tempdir().expect("upload dir");
        for dir in ["tmp", "artists"] {
            std::fs::create_dir_all(upload_dir.path().join(dir)).expect("uploads subdirectory");
        }
        let server = api_server_with_uploads(&pool, upload_dir.path().to_path_buf()).await;

        let response = post_form(
            &server,
            &form_body(
                &[("file_type", "Artist"), ("slug", "test-artist-1")],
                Some(("image/png", "cover.png", PNG_BODY)),
            ),
        )
        .await;

        response.assert_status(axum::http::StatusCode::SEE_OTHER);
        assert_eq!(location(&response), "/login");
        assert!(
            std::fs::read_dir(upload_dir.path().join("artists"))
                .expect("artists dir readable")
                .next()
                .is_none(),
            "an unauthenticated post must not store anything"
        );
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn upload_form_rejects_a_mime_the_configuration_disallows(pool: PgPool) {
        let upload_dir = tempfile::tempdir().expect("upload dir");
        let server = signed_in_server(&pool, upload_dir.path()).await;

        let response = post_form(
            &server,
            &form_body(
                &[("file_type", "Artist"), ("slug", "test-artist-1")],
                Some(("text/plain", "notes.txt", b"not a picture")),
            ),
        )
        .await;

        response.assert_status(axum::http::StatusCode::SEE_OTHER);
        assert_eq!(
            location(&response),
            "/admin/artist/test-artist-1/images?bad_upload=bad_mime"
        );
        assert!(
            std::fs::read_dir(upload_dir.path().join("artists"))
                .expect("artists dir readable")
                .next()
                .is_none(),
            "a rejected mime must not store anything"
        );
    }

    /// A part may arrive claiming `image/png` while holding anything at all: a
    /// browser takes that claim from the file's name, so the bytes have to be
    /// checked before the file is moved into place.
    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn upload_form_rejects_bytes_that_are_not_an_image(pool: PgPool) {
        let upload_dir = tempfile::tempdir().expect("upload dir");
        let server = signed_in_server(&pool, upload_dir.path()).await;

        let response = post_form(
            &server,
            &form_body(
                &[("file_type", "Artist"), ("slug", "test-artist-1")],
                Some(("image/png", "portrait.png", b"a letter to the label")),
            ),
        )
        .await;

        response.assert_status(axum::http::StatusCode::SEE_OTHER);
        assert_eq!(
            location(&response),
            "/admin/artist/test-artist-1/images?bad_upload=bad_image"
        );
        assert!(
            std::fs::read_dir(upload_dir.path().join("artists"))
                .expect("artists dir readable")
                .next()
                .is_none(),
            "wrong bytes must not be stored"
        );
        assert!(
            std::fs::read_dir(upload_dir.path().join("tmp"))
                .expect("tmp dir readable")
                .next()
                .is_none(),
            "the temp file is undone"
        );
        let artist = Artist::get_by_slug(&pool, "test-artist-1".to_string())
            .await
            .expect("artist after the rejected upload");
        assert_eq!(artist.primary_image, None, "and the row is left alone");
    }

    /// `signed_in_server` plus a release and a track under `test-artist-1`, and
    /// the `tracks` directory the audio tail writes into.
    #[expect(
        clippy::future_not_send,
        reason = "awaits the same !Send axum-test helper as `signed_in_server`; callers are `#[sqlx::test]` bodies on one thread"
    )]
    async fn audio_server(pool: &PgPool, upload_root: &std::path::Path) -> axum_test::TestServer {
        let server = signed_in_server(pool, upload_root).await;
        std::fs::create_dir_all(upload_root.join("tracks")).expect("uploads subdirectory");
        let artist = Artist::get_by_slug(pool, "test-artist-1".to_string())
            .await
            .expect("fixture artist");
        let release = create_test_release(pool, 1, Some(artist.clone()))
            .await
            .expect("fixture release");
        create_test_track(pool, 1, Some(release), Some(artist))
            .await
            .expect("fixture track");
        server
    }

    /// A track's audio goes through the same form as an image. The part's
    /// declared type here is `audio/x-flac`, a legacy spelling a browser's own
    /// mime table really does answer with: the extension carries the claim, and
    /// the `fLaC` bytes decide what is stored.
    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future in a `#[sqlx::test]` body"
    )]
    #[sqlx::test]
    async fn upload_form_stores_track_audio_and_points_at_its_editor(pool: PgPool) {
        let upload_dir = tempfile::tempdir().expect("upload dir");
        let server = audio_server(&pool, upload_dir.path()).await;

        let response = post_form(
            &server,
            &form_body(
                &[
                    ("file_type", "TrackAudio"),
                    ("slug", "test-track-1"),
                    ("artist_slug", "test-artist-1"),
                    ("release_slug", "test-release-1"),
                ],
                Some(("audio/x-flac", "castles.flac", FLAC_BODY)),
            ),
        )
        .await;

        response.assert_status(axum::http::StatusCode::SEE_OTHER);
        assert_eq!(
            location(&response),
            "/admin/artist/test-artist-1/release/test-release-1/track/test-track-1?uploaded=1"
        );

        let track = Track::find_by_slug(&pool, "test-track-1".to_string())
            .await
            .expect("the stored track")
            .expect("the track the upload addressed");
        let stored = track
            .audio
            .as_deref()
            .expect("the audio column points at the upload");
        assert!(
            stored.ends_with("-test-track-1.flac"),
            "audio is named after its track: {stored}"
        );
        let stored_path = upload_dir.path().join("tracks").join(stored);
        assert_eq!(
            std::fs::read(stored_path).expect("the stored file"),
            FLAC_BODY,
            "the bytes landed in the tracks directory"
        );
        assert_eq!(
            track.audio_url().as_deref(),
            Some(format!("/uploads/tracks/{stored}").as_str()),
            "and the model turns the column into a playable URL"
        );
    }

    /// The audio slot takes audio, and a picture is caught at the claim check:
    /// neither the part's own type nor its `.png` extension is one the slot
    /// takes, so the upload comes back with a code and nothing stored.
    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future in a `#[sqlx::test]` body"
    )]
    #[sqlx::test]
    async fn upload_form_rejects_a_picture_for_the_audio_slot(pool: PgPool) {
        let upload_dir = tempfile::tempdir().expect("upload dir");
        let server = audio_server(&pool, upload_dir.path()).await;

        let response = post_form(
            &server,
            &form_body(
                &[
                    ("file_type", "TrackAudio"),
                    ("slug", "test-track-1"),
                    ("artist_slug", "test-artist-1"),
                    ("release_slug", "test-release-1"),
                ],
                Some(("image/png", "sleeve.png", PNG_BODY)),
            ),
        )
        .await;

        response.assert_status(axum::http::StatusCode::SEE_OTHER);
        assert_eq!(
            location(&response),
            "/admin/artist/test-artist-1/release/test-release-1/track/test-track-1\
             ?bad_upload=bad_mime"
        );
        assert!(
            std::fs::read_dir(upload_dir.path().join("tracks"))
                .expect("tracks dir readable")
                .next()
                .is_none(),
            "a rejected audio upload stores nothing"
        );
        let track = Track::find_by_slug(&pool, "test-track-1".to_string())
            .await
            .expect("the stored track")
            .expect("the track row");
        assert_eq!(track.audio, None, "and the row is left alone");
    }

    #[test]
    fn upload_targets_are_the_pages_that_host_the_widget() {
        assert_eq!(
            upload_target(UploadConfiguration::Avatar, "test-1", "", "").as_deref(),
            Some("/profile")
        );
        assert_eq!(
            upload_target(UploadConfiguration::Artist, "test-artist-1", "", "").as_deref(),
            Some("/admin/artist/test-artist-1/images")
        );
        assert_eq!(
            upload_target(
                UploadConfiguration::Release,
                "test-release-1",
                "test-artist-1",
                ""
            )
            .as_deref(),
            Some("/admin/artist/test-artist-1/release/test-release-1")
        );
        // A track editor is addressed by three slugs, so its audio upload has
        // to carry all three to come back to the page it came from.
        assert_eq!(
            upload_target(
                UploadConfiguration::TrackAudio,
                "test-track-1",
                "test-artist-1",
                "test-release-1"
            )
            .as_deref(),
            Some("/admin/artist/test-artist-1/release/test-release-1/track/test-track-1")
        );
        // A release needs both slugs, a track all three, and nothing else hosts
        // the widget.
        assert!(upload_target(UploadConfiguration::Release, "test-release-1", "", "").is_none());
        assert!(
            upload_target(
                UploadConfiguration::TrackAudio,
                "test-track-1",
                "test-artist-1",
                ""
            )
            .is_none()
        );
    }

    #[test]
    fn only_slugify_alphabet_slugs_get_a_redirect_target() {
        assert!(canonical_slug("test-artist-1"));
        assert!(canonical_slug("a1"));
        // Hand-crafted bodies must not steer the Location header.
        assert!(!canonical_slug(""));
        assert!(!canonical_slug("../elsewhere"));
        assert!(!canonical_slug("jimi/hendrix"));
        assert!(!canonical_slug("Jimi"));
        assert!(!canonical_slug("jimi\nLocation: http://example.com"));
    }

    #[test]
    fn upload_codes_cover_the_known_failures() {
        assert_eq!(upload_code(&AppError::Forbidden("no".into())), "forbidden");
        assert_eq!(
            upload_code(&AppError::Unauthorized("User not supplied.".into())),
            "forbidden"
        );
        assert_eq!(
            upload_code(&AppError::NotFound("no row".into())),
            "not_found"
        );
        assert_eq!(
            upload_code(&AppError::BadRequest("nope".into())),
            "bad_form"
        );
        assert_eq!(upload_code(&AppError::Internal("boom".into())), "server");
    }
}
