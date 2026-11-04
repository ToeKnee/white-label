//! File endpoints ported from `routes/files/upload.rs` (see the `api`
//! module docs for the base wire protocol). These two are the specials:
//!
//! - `POST /api/upload_file` uses the `MultipartFormData` input codec: the
//!   body is raw `multipart/form-data` and the Leptos handler consumes
//!   fields *in order* (1: upload configuration, e.g. `Artist`; 2: slug;
//!   3+: file fields), so the field names are ignored — reproduced here.
//!   Success answers `200` with the JSON `null` (the `()` output).
//! - `POST /api/file_progress` keeps the usual form-encoded request but
//!   answers a `text/plain` stream of `"<total bytes>\n"` lines that stays
//!   open until the matching upload finishes (the upload handler drops the
//!   `FILES` registry entry, closing the broadcast channel).
//!
//! Deviation: the Leptos route reads `UPLOAD_PATH` from the environment
//! inside the handler; here the root comes from the [`UploadRoot`] request
//! extension that `build_router` installs — same value in production,
//! injectable in tests.
//!
//! The storage tail (move the temp file into place, attach it to the row) is
//! shared with the dioxus upload server fn in [`crate::ui::upload`] via
//! [`crate::services::files::upload::finalise_upload`].

use axum::body::Body;
use axum::extract::Extension;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use futures::{StreamExt as _, TryStreamExt as _};
use std::io::Write as _;

use crate::config::upload::UploadConfiguration;
use crate::routes::files::upload::FileProgress as FileProgressArgs;
use crate::server::UploadRoot;
use crate::server::api::{WireError, decode_args, json_response};
use crate::server::current_user::{CurrentUser, Db};
use crate::services::authentication_helpers::user_with_permissions;
use crate::services::files::progress::{FILES, add_chunk, progress_for_file};
use crate::services::files::upload::{UploadedFile, finalise_upload};
use crate::utils::files::valid_file_name;

const PATH_UPLOAD_FILE: &str = "/api/upload_file";
const PATH_FILE_PROGRESS: &str = "/api/file_progress";

/// Streamed upload: parse the multipart body and save the file, mirroring
/// the Leptos `upload_file` route step by step (including its flattened
/// error messages).
pub async fn upload_file(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    Extension(upload_root): Extension<UploadRoot>,
    request: Request,
) -> Result<Response, WireError> {
    let boundary = multer::parse_boundary(
        request
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default(),
    )
    .map_err(|error| WireError::args(PATH_UPLOAD_FILE, error))?;
    let mut multipart = multer::Multipart::new(
        request
            .into_body()
            .into_data_stream()
            .map_err(|error| multer::Error::StreamReadFailed(Box::new(error))),
        boundary,
    );

    // Field order is the protocol (see module docs).
    let upload_config_type = next_field_text(&mut multipart, "upload configuration").await?;
    let upload_config = upload_config_type
        .parse::<UploadConfiguration>()
        .map_err(|_| WireError::internal(PATH_UPLOAD_FILE, "Invalid upload configuration."))?;
    let upload_details = upload_config.get_details();
    let slug_field = next_field_text(&mut multipart, "slug").await?;

    let permissions = upload_details
        .permissions
        .iter()
        .map(String::as_str)
        .collect();
    let user = user_with_permissions(current_user.as_ref(), permissions)
        .map_err(|error| WireError::from_app_error(PATH_UPLOAD_FILE, &error))?;

    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|error| WireError::internal(PATH_UPLOAD_FILE, error))?
    {
        // Every file field must declare an allowed content type.
        let Some(content_type) = field.content_type() else {
            return Err(WireError::internal(
                PATH_UPLOAD_FILE,
                "No content type on field.",
            ));
        };
        if !upload_details
            .mime_types
            .contains(&content_type.to_string())
        {
            return Err(WireError::internal(PATH_UPLOAD_FILE, "Invalid mime type."));
        }
        let Some(original_file_name) = field.file_name() else {
            return Err(WireError::internal(
                PATH_UPLOAD_FILE,
                "No filename on field.",
            ));
        };
        let original_file_name = original_file_name.to_string();

        let file_name = valid_file_name(
            &original_file_name,
            Some(slug_field.clone()),
            &upload_details.path,
            true,
        )
        .map_err(|error| WireError::internal(PATH_UPLOAD_FILE, error))?;

        let upload_path = upload_root
            .root()
            .ok_or_else(|| WireError::internal(PATH_UPLOAD_FILE, "No upload path specified."))?;
        let tmp_path = format!("{upload_path}/tmp/{file_name}");
        tracing::info!("Uploading {file_name} to {tmp_path}");
        let mut file = std::fs::File::create(tmp_path.clone())
            .map_err(|error| WireError::internal(PATH_UPLOAD_FILE, error))?;

        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|error| WireError::internal(PATH_UPLOAD_FILE, error))?
        {
            let total_so_far = add_chunk(&original_file_name, chunk.len(), &user.username).await;
            if total_so_far > upload_details.size_limit {
                // Undo the partial upload and its progress entry.
                std::fs::remove_file(tmp_path.clone())
                    .map_err(|error| WireError::internal(PATH_UPLOAD_FILE, error))?;
                FILES.remove(&format!("{}-{original_file_name}", user.username));
                return Err(WireError::internal(PATH_UPLOAD_FILE, "File too large."));
            }
            file.write_all(&chunk)
                .map_err(|error| WireError::internal(PATH_UPLOAD_FILE, error))?;
        }

        // Field complete: move the file into place, then attach it.
        finalise_upload(
            &pool,
            &upload_path,
            upload_config,
            UploadedFile {
                tmp_path,
                file_name,
                original_file_name,
            },
            &user,
            &slug_field,
        )
        .await
        .map_err(|error| WireError::internal(PATH_UPLOAD_FILE, error))?;
    }

    json_response(PATH_UPLOAD_FILE, &())
}

/// Read the next multipart field as text (the ordered header fields).
async fn next_field_text(
    multipart: &mut multer::Multipart<'_>,
    purpose: &str,
) -> Result<String, WireError> {
    let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| WireError::internal(PATH_UPLOAD_FILE, error))?
    else {
        return Err(WireError::internal(
            PATH_UPLOAD_FILE,
            "No fields in request.",
        ));
    };
    field
        .text()
        .await
        .map_err(|error| WireError::internal(PATH_UPLOAD_FILE, error))
        .map(|text| {
            tracing::debug!("upload {purpose} field: {text}");
            text
        })
}

/// Progress of an in-flight upload as a `text/plain` stream of byte totals
/// (`admin` + `label_owner` only, as in the Leptos route).
pub async fn file_progress(
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<FileProgressArgs>(PATH_FILE_PROGRESS, &body)?;

    let user = user_with_permissions(current_user.as_ref(), vec!["admin", "label_owner"])
        .map_err(|error| WireError::from_app_error(PATH_FILE_PROGRESS, &error))?;

    tracing::debug!("Getting progress on {}", args.filename);
    // Each total goes on its own line; the HTTP layer may pack several
    // lines into one chunk — the Leptos client splits them again.
    let progress = progress_for_file(&args.filename, &user.username)
        .map(|total| Ok::<_, std::convert::Infallible>(format!("{total}\n")));

    let response = (
        StatusCode::OK,
        [(
            CONTENT_TYPE,
            axum::http::HeaderValue::from_static("text/plain"),
        )],
        Body::from_stream(progress),
    )
        .into_response();
    Ok(response)
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::{PATH_FILE_PROGRESS, PATH_UPLOAD_FILE};
    use crate::models::artist::Artist;
    use crate::models::test_helpers::{
        create_test_artist, create_test_record_label, create_test_user_with_permissions,
    };
    use crate::routes::auth::Login;
    use crate::routes::files::upload::FileProgress as FileProgressArgs;
    use crate::server::api::test_support::{api_server, api_server_with_uploads, post_args};
    use crate::server::current_user::CurrentUser;

    const BOUNDARY: &str = "white-label-test-boundary";

    /// Body in the exact field order the Leptos upload client sends:
    /// configuration, slug, then the file part.
    fn multipart_body(config: &str, slug: &str, file: Option<(&str, &str, &[u8])>) -> Vec<u8> {
        let mut body = Vec::new();
        for (name, value) in [("type", config), ("slug", slug)] {
            body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
            body.extend_from_slice(
                format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
            );
            body.extend_from_slice(value.as_bytes());
            body.extend_from_slice(b"\r\n");
        }
        if let Some((name, content_type, bytes)) = file {
            body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
            body.extend_from_slice(
                format!(
                    "Content-Disposition: form-data; name=\"{name}\"; filename=\"{name}.png\"\r\n\
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

    fn post_multipart(server: &axum_test::TestServer, body: &[u8]) -> axum_test::TestRequest {
        server
            .post(PATH_UPLOAD_FILE)
            .content_type(&format!("multipart/form-data; boundary={BOUNDARY}"))
            .bytes(axum::body::Bytes::copy_from_slice(body))
    }

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
    async fn label_owner_uploads_an_artist_image(pool: PgPool) {
        let label = create_test_record_label(&pool, 1).await.expect("label");
        create_test_artist(&pool, 1, Some(label))
            .await
            .expect("artist");
        create_test_user_with_permissions(&pool, 1, vec!["admin", "label_owner"])
            .await
            .expect("admin + label_owner user");

        let upload_dir = tempfile::tempdir().expect("upload dir");
        // The Leptos app expects these to exist; create them like setup does.
        std::fs::create_dir_all(upload_dir.path().join("tmp")).expect("tmp dir");
        std::fs::create_dir_all(upload_dir.path().join("artists")).expect("artists dir");

        let server = api_server_with_uploads(&pool, upload_dir.path().to_path_buf()).await;
        post_args(&server, "/api/login", &login_args("test-1"))
            .await
            .assert_status_ok();

        let response = post_multipart(
            &server,
            &multipart_body(
                "Artist",
                "test-artist-1",
                Some(("picture", "image/png", b"png bytes")),
            ),
        )
        .await;

        response.assert_status_ok();
        assert_eq!(response.text(), "null"); // () over the Json codec

        // The file moved out of tmp/ and the artist row now names it.
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
            b"png bytes"
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
    async fn upload_file_without_login_is_unauthorized(pool: PgPool) {
        let upload_dir = tempfile::tempdir().expect("upload dir");
        std::fs::create_dir_all(upload_dir.path().join("tmp")).expect("tmp dir");
        let server = api_server_with_uploads(&pool, upload_dir.path().to_path_buf()).await;

        let response = post_multipart(
            &server,
            &multipart_body(
                "Artist",
                "test-artist-1",
                Some(("picture", "image/png", b"x")),
            ),
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
    async fn upload_file_with_bad_config_is_internal(pool: PgPool) {
        let server = api_server(pool).await;

        let response = post_multipart(&server, &multipart_body("Bogus", "whatever", None)).await;

        response.assert_status_internal_server_error();
        assert_eq!(response.text(), "ServerError|Invalid upload configuration.");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn upload_file_rejects_disallowed_mime_type(pool: PgPool) {
        create_test_user_with_permissions(&pool, 4, vec!["admin", "label_owner"])
            .await
            .expect("admin + label_owner user");
        let upload_dir = tempfile::tempdir().expect("upload dir");
        std::fs::create_dir_all(upload_dir.path().join("tmp")).expect("tmp dir");
        let server = api_server_with_uploads(&pool, upload_dir.path().to_path_buf()).await;

        post_args(&server, "/api/login", &login_args("test-4"))
            .await
            .assert_status_ok();

        let response = post_multipart(
            &server,
            &multipart_body(
                "Artist",
                "test-artist-4",
                Some(("notes", "text/plain", b"hi")),
            ),
        )
        .await;

        response.assert_status_internal_server_error();
        assert_eq!(response.text(), "ServerError|Invalid mime type.");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn file_progress_without_login_is_unauthorized(pool: PgPool) {
        let server = api_server(pool).await;

        let response = post_args(
            &server,
            PATH_FILE_PROGRESS,
            &FileProgressArgs {
                filename: "some.png".to_string(),
            },
        )
        .await;

        response.assert_status_unauthorized();
        assert_eq!(response.text(), "ServerError|User not supplied.");
    }

    #[sqlx::test]
    async fn file_progress_streams_chunk_totals(pool: PgPool) {
        use futures::StreamExt as _;

        let user = create_test_user_with_permissions(&pool, 9, vec!["admin", "label_owner"])
            .await
            .expect("admin + label_owner user");

        // The handler must be called before add_chunk: that creates the
        // broadcast entry the stream listens on (same registry the real
        // upload populates).
        let body = serde_qs::to_string(&FileProgressArgs {
            filename: "progress.png".to_string(),
        })
        .expect("args should serialize");
        let response = super::file_progress(CurrentUser(Some(user.clone())), body)
            .await
            .expect("progress should stream for an admin + label_owner user");
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::CONTENT_TYPE)
                .expect("content type")
                .to_str()
                .expect("ascii"),
            "text/plain"
        );

        crate::services::files::progress::add_chunk("progress.png", 5, &user.username).await;

        let mut data = response.into_body().into_data_stream();
        let first = data
            .next()
            .await
            .expect("stream should end only when the upload does")
            .expect("body chunk");
        assert_eq!(first, "5\n");

        crate::services::files::progress::add_chunk("progress.png", 3, &user.username).await;
        let second = data
            .next()
            .await
            .expect("second chunk")
            .expect("body chunk");
        assert_eq!(second, "8\n");

        // The upload finishing (finalise removes the registry entry) is
        // what closes the stream.
        crate::services::files::progress::FILES.remove(&format!("{}-progress.png", user.username));
        assert!(
            data.next().await.is_none(),
            "stream should end when the upload completes"
        );
    }
}
