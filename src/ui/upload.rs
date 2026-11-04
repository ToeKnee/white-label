//! The dioxus upload widget and its endpoints (MIGRATION.md Phase 5 slice
//! 5f).
//!
//! A `FileStream` argument *is* the request body: dioxus sends the file's
//! own mime type in `Content-Type`, its size in `X-Content-Size` and its
//! name in `Content-Disposition`, and only one body argument is possible, so
//! the scalars are declared in the route literal and travel as query params
//! (`/api/upload?file_type=Avatar&slug=…&filename=…`).
//!
//! Upload and progress are two concurrent requests because a browser will
//! not duplex one (`services::files::progress` carries the same note).
//! Progress totals live in that registry, keyed by the session username plus
//! the filename; the widget sends the filename itself rather than relying on
//! the `Content-Disposition` copy, which dioxus escapes client-side and leaves
//! percent-encoded on the server.
//!
//! Deviations from the Leptos original:
//! - No plain-HTML fallback: `/api/upload` speaks dioxus' body protocol, so an
//!   upload needs the client build. The Leptos widget was javascript-only too,
//!   but it left a form-post twin behind; there is none here.
//! - [`file_progress`](crate::ui::upload::file_progress) needs only a session,
//!   not `admin` + `label_owner`: the
//!   registry key already carries the requester's username, so the extra
//!   check did nothing except hide the avatar progress bar behind a 403 for
//!   everyone without those permissions. The legacy wire endpoint keeps its
//!   original check for wire parity.
//! - [`upload_file`](crate::ui::upload::upload_file) answers the stored file
//!   name, which the widget shows;
//!   the Leptos route answered nothing and its client logged a TODO about
//!   refreshing the page.

use dioxus::fullstack::{FileStream, TextStream};
use dioxus::html::FileData;
use dioxus::prelude::*;

use crate::config::upload::UploadConfiguration;
use crate::ui::utils::{InlineError, Success};

#[cfg(feature = "server")]
use crate::services::authentication_helpers::user_with_permissions;
#[cfg(feature = "server")]
use crate::services::files::progress::{FILES, add_chunk, progress_for_file};
#[cfg(feature = "server")]
use crate::services::files::upload::{UploadedFile, finalise_upload};
#[cfg(feature = "server")]
use crate::ui::data::to_http_error;
#[cfg(feature = "server")]
use crate::utils::files::valid_file_name;
#[cfg(feature = "server")]
use dioxus::fullstack::Streaming;
#[cfg(feature = "server")]
use futures::StreamExt as _;
#[cfg(feature = "server")]
use std::io::Write as _;

/// Uploads the request body as one file for `file_type` and attaches it to
/// `slug` (an artist or release slug; `Avatar` takes the uploader's own
/// username).
///
/// Success answers the name the file was stored under.
///
/// The bytes stream to `<root>/tmp`, counted for the progress endpoint, and
/// the shared `services::files::upload` tail moves them into the config's
/// directory and updates the row — the same tail the Leptos wire endpoint
/// uses, so storage and attachment cannot drift.
///
/// # Errors
/// 400 for an unknown configuration or a rejected filename, 401 with no
/// session, 403 when the configuration needs permissions the session lacks,
/// 415 for a mime type the configuration disallows, 413 once the stream
/// passes the size limit, 500 when the uploads root is missing or the write
/// fails.
#[post("/api/upload?file_type&slug&filename",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser,
    upload_root: axum::Extension<crate::server::UploadRoot>
)]
pub async fn upload_file(
    file_type: String,
    slug: String,
    filename: String,
    mut file_stream: FileStream,
) -> core::result::Result<String, HttpError> {
    let Ok(upload_config) = file_type.parse::<UploadConfiguration>() else {
        return HttpError::bad_request("Invalid upload configuration.");
    };
    let details = upload_config.get_details();

    let Some(session_user) = user.0 else {
        return HttpError::unauthorized("User not supplied.");
    };
    let required_permissions = details.permissions.iter().map(String::as_str).collect();
    let uploader = match user_with_permissions(Some(&session_user), required_permissions) {
        Ok(uploader) => uploader,
        Err(error) => return to_http_error(&error),
    };

    let mime_type = file_stream.content_type().unwrap_or_default();
    if !details
        .mime_types
        .iter()
        .any(|allowed| allowed == mime_type)
    {
        return HttpError::unsupported_media_type("Invalid mime type.");
    }
    // `filename` is the client's own copy (see the module docs); the
    // Content-Disposition name would arrive percent-encoded. The last
    // argument keeps the Leptos behaviour of overwriting an existing file.
    let tmp_file_name = match valid_file_name(&filename, Some(slug.clone()), &details.path, true) {
        Ok(valid_name) => valid_name,
        Err(error) => {
            tracing::error!("upload rejected ({filename}): {error}");
            return HttpError::bad_request("Invalid file name.");
        }
    };
    let Some(upload_root) = upload_root.0.root() else {
        return HttpError::internal_server_error("No upload path specified.");
    };

    let tmp_path = format!("{upload_root}/tmp/{tmp_file_name}");
    tracing::info!("Uploading {tmp_file_name} to {tmp_path}");
    let size_limit = details.size_limit;
    let stream_result = async {
        let mut tmp_file = std::fs::File::create(&tmp_path).map_err(|error| {
            tracing::error!("cannot create {tmp_path}: {error}");
            HttpError::new(
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "Upload write failed.",
            )
        })?;
        let mut received: usize = 0;
        while let Some(chunk) = file_stream.next().await {
            let chunk = chunk.map_err(|error| {
                tracing::error!("upload stream for {tmp_file_name} failed: {error}");
                HttpError::new(http::StatusCode::BAD_REQUEST, "Upload stream error.")
            })?;
            if received + chunk.len() > size_limit {
                return Err(HttpError::new(
                    http::StatusCode::PAYLOAD_TOO_LARGE,
                    format!(
                        "File too large (the limit is {} MB).",
                        size_limit / (1024 * 1024)
                    ),
                ));
            }
            tmp_file.write_all(&chunk).map_err(|error| {
                tracing::error!("cannot write {tmp_path}: {error}");
                HttpError::new(
                    http::StatusCode::INTERNAL_SERVER_ERROR,
                    "Upload write failed.",
                )
            })?;
            received += chunk.len();
            add_chunk(&filename, chunk.len(), &uploader.username).await;
        }
        Ok::<(), HttpError>(())
    }
    .await;

    if let Err(error) = stream_result {
        // Undo the partial upload, and let any progress stream end.
        if let Err(error) = std::fs::remove_file(&tmp_path) {
            tracing::error!("could not remove the partial upload {tmp_path}: {error}");
        }
        FILES.remove(format!("{}-{filename}", uploader.username).as_str());
        return Err(error);
    }

    finalise_upload(
        &db.0,
        &upload_root,
        upload_config,
        UploadedFile {
            tmp_path,
            file_name: tmp_file_name.clone(),
            original_file_name: filename,
        },
        &uploader,
        &slug,
    )
    .await
    .map_err(|error| {
        tracing::error!("upload {tmp_file_name} could not be finished: {error}");
        HttpError::new(
            http::StatusCode::INTERNAL_SERVER_ERROR,
            "Upload could not be finished.",
        )
    })
}

/// The running byte totals of the caller's own in-flight upload, one per
/// line, until that upload finishes (which is what closes the stream).
///
/// # Errors
/// 401 with no session.
// The dioxus macro awaits this handler, so it has to be `async` even though
// the body itself is synchronous (the totals stream is handed over uncalled).
#[cfg_attr(
    feature = "server",
    expect(
        clippy::unused_async,
        reason = "the dioxus `#[get]` macro awaits the handler; the body has no await of its own"
    )
)]
#[get("/api/file_progress?filename", user: crate::server::current_user::CurrentUser)]
pub async fn file_progress(filename: String) -> core::result::Result<TextStream, HttpError> {
    let Some(session_user) = user.0 else {
        return HttpError::unauthorized("User not supplied.");
    };
    // The registry key is the requester's own username, so a session can
    // only ever watch its own uploads.
    let totals =
        progress_for_file(&filename, &session_user.username).map(|total| format!("{total}\n"));
    Ok(Streaming::new(totals))
}

// ---------------------------------------------------------------------------
// The widget
// ---------------------------------------------------------------------------

/// File picker, Upload button and progress bar (the Leptos
/// `FileUploadWithProgress`). Only the wasm client can upload, so SSR shows
/// the controls without a fallback target.
#[component]
pub fn FileUploadWithProgress(
    /// Which slot to upload to: chooses the directory, mime types, size limit
    /// and required permissions (see [`UploadConfiguration::get_details`]).
    config: UploadConfiguration,
    /// The item the file belongs to: an artist or release slug, or the
    /// uploader's own username for [`UploadConfiguration::Avatar`].
    slug: String,
    /// Runs after a file is stored, with the name it was stored under. The
    /// artist and release pages use it to point their `<img>` at the new
    /// file, so the user does not have to reload (Leptos left the old image
    /// on screen until they did).
    #[props(optional)]
    on_uploaded: Option<EventHandler<String>>,
) -> Element {
    let details = config.get_details();
    let accepted_types = details.mime_types.join(",");
    let size_limit_mb = details.size_limit / (1024 * 1024);
    // `max`/`value` are written as plain attributes: dioxus types them as
    // `f64` on `<progress>`, and byte counts read better as strings.
    let mut uploading = use_signal(|| None::<String>);
    let mut max_bytes = use_signal(|| None::<u64>);
    let mut sent_bytes = use_signal(|| None::<u64>);

    // Clone the slug *outside* the future: the action's future is spawned,
    // so it has to own its data rather than borrow the closure.
    let mut submit = use_action(move |file: FileData| {
        let slug = slug.clone();
        async move {
            let attempt = upload_attempt(config, slug, file, sent_bytes, size_limit_mb).await;
            // Tell the page about the stored file before the alert renders,
            // so its image swap and the alert arrive in one pass.
            if let (Some(callback), Ok(outcome)) = (on_uploaded, attempt.as_ref())
                && let Some(stored_name) = outcome.stored_name.clone()
            {
                callback.call(stored_name);
            }
            attempt
        }
    });

    let outcome = match submit.value() {
        Some(Ok(outcome)) => Some(outcome.cloned()),
        // Unreachable: this action reports every failure inside the outcome.
        Some(Err(captured)) => Some(UploadOutcome {
            succeeded: false,
            stored_name: None,
            message: format!("Upload failed: {captured}"),
        }),
        None => None,
    };
    let in_flight = submit.pending();
    let show_progress = in_flight || (max_bytes().is_some() && outcome.is_none());

    rsx! {
        form {
            onsubmit: move |evt| {
                // The upload protocol is a request body, not a form post.
                evt.prevent_default();
                sent_bytes.set(None);
                if let Some(file) = picked_file(&evt) {
                    uploading.set(Some(file.name()));
                    max_bytes.set(Some(file.size()));
                    submit.call(file);
                }
            },
            div {
                class: "grid gap-6",
                div {
                    class: "flex flex-auto gap-6",
                    input {
                        r#type: "file",
                        name: "file_to_upload",
                        class: "w-full file-input file-input-primary",
                        accept: accepted_types,
                    }
                }
                div {
                    class: "flex flex-auto gap-6",
                    button { class: "flex-1 btn btn-primary", r#type: "submit", "Upload" }
                }
            }
        }
        if let Some(outcome) = outcome {
            if outcome.succeeded {
                Success { message: outcome.message, show: true }
            } else {
                InlineError { message: outcome.message }
            }
        }
        if show_progress {
            if in_flight {
                if let Some(filename) = uploading() {
                    p { "Uploading {filename}" }
                }
            }
            if let Some(max) = max_bytes() {
                progress {
                    class: "w-56 progress progress-primary",
                    max: bytes_for_progress(max),
                    value: bytes_for_progress(sent_bytes().unwrap_or_default()),
                }
            }
        }
    }
}

/// The file chosen in `file_to_upload`. Browsers submit an untouched file
/// input as a file with an empty name, which is not an upload.
fn picked_file(evt: &FormEvent) -> Option<FileData> {
    match evt.get_first("file_to_upload") {
        Some(FormValue::File(file)) => file.filter(|file| !file.name().is_empty()),
        _ => None,
    }
}

/// Byte counts as the `f64` that dioxus types `<progress>` max/value with.
/// The config caps are hundreds of megabytes at most, so nothing is lost.
#[expect(
    clippy::cast_precision_loss,
    reason = "dioxus types `<progress>` max/value as f64; upload limits stay far below 2^53 bytes"
)]
const fn bytes_for_progress(bytes: u64) -> f64 {
    bytes as f64
}

/// What one upload attempt wants to tell the user about.
#[derive(Clone)]
struct UploadOutcome {
    succeeded: bool,
    /// The name the file was stored under (set only on success); pages use
    /// it to build the file's URL.
    stored_name: Option<String>,
    message: String,
}

/// The public URL an upload is served from, matching the models'
/// `primary_image_url` (`/uploads/<config directory>/<stored name>`).
#[must_use]
pub fn stored_file_url(config: UploadConfiguration, stored_name: &str) -> String {
    format!("/uploads/{}/{stored_name}", config.get_details().path)
}

/// Runs one attempt: the progress request is established before the file
/// body goes up, so the totals entry is registered before any chunk lands.
///
/// The two requests then run side by side. If the upload finishes before its
/// stream closes, the stream is dropped rather than waited on.
///
/// Everything the user needs to see travels in [`UploadOutcome`], so the
/// action itself cannot fail (`Infallible` keeps dioxus' action signature
/// honest and stops a `CapturedError` from swallowing the message).
// The two signals this helper writes are dioxus `Signal`s, which are
// deliberately `!Send` (they live on the component's local executor), so the
// future can never be `Send`; it is only ever awaited on that executor.
#[expect(
    clippy::future_not_send,
    reason = "takes dioxus Signals, which are !Send by design; this future only ever runs on the component's executor"
)]
async fn upload_attempt(
    config: UploadConfiguration,
    slug: String,
    file: FileData,
    mut sent_bytes: Signal<Option<u64>>,
    size_limit_mb: usize,
) -> core::result::Result<UploadOutcome, std::convert::Infallible> {
    let filename = file.name();
    let size_bytes = file.size();
    let details = config.get_details();

    // Cheap checks first, so an obviously doomed upload never asks for a
    // progress stream at all.
    if size_bytes > u64::try_from(details.size_limit).unwrap_or(u64::MAX) {
        return Ok(upload_outcome(HttpError::payload_too_large(format!(
            "{filename} is larger than the {size_limit_mb} MB limit."
        ))));
    }
    let mime_type = file.content_type().unwrap_or_default();
    if !details.mime_types.contains(&mime_type) {
        return Ok(upload_outcome(HttpError::unsupported_media_type(format!(
            "{filename} is not a supported file type."
        ))));
    }

    let progress_stream = match file_progress(filename.clone()).await {
        Ok(totals) => Some(totals),
        Err(error) => {
            tracing::error!("could not start the progress stream for {filename}: {error}");
            None
        }
    };
    // Two futures on one task: the file body and the totals stream are
    // interleaved at their await points, which is how the bar moves.
    let drain_totals = drain_progress(progress_stream, sent_bytes, filename.clone());

    let upload = upload_file(config.to_string(), slug, filename, FileStream::from(file));
    futures::pin_mut!(drain_totals, upload);
    let result = match futures::future::select(drain_totals, upload).await {
        // The stream closed when the upload finished: wait for its answer.
        futures::future::Either::Left(((), upload)) => upload.await,
        // The upload is done (a fast rejection, usually); stop listening.
        futures::future::Either::Right((result, _totals)) => {
            sent_bytes.set(Some(size_bytes));
            result
        }
    };
    Ok(upload_outcome(result))
}

/// Copies the upload's running byte totals into `sent_bytes`.
///
/// The stream ends by itself when the upload finishes (that is what drops its
/// progress entry), and the last line of a chunk is the current total because
/// the HTTP layer may pack several totals into one chunk.
#[expect(
    clippy::future_not_send,
    reason = "takes a dioxus Signal, which is !Send by design; this future only ever runs on the component's executor"
)]
async fn drain_progress(
    totals: Option<TextStream>,
    mut sent_bytes: Signal<Option<u64>>,
    filename: String,
) {
    let Some(mut totals) = totals else { return };
    while let Some(total) = totals.next().await {
        match total {
            Ok(chunk) => {
                let Some(last_line) = chunk.split('\n').rfind(|line| !line.is_empty()) else {
                    continue;
                };
                // A line split across chunks is skipped here; the next
                // chunk completes it.
                if let Ok(bytes) = last_line.parse::<u64>() {
                    sent_bytes.set(Some(bytes));
                }
            }
            Err(error) => {
                tracing::error!("the progress stream for {filename} ended: {error}");
                break;
            }
        }
    }
}

/// Turns an upload's transport result into the widget's alert text.
fn upload_outcome(result: core::result::Result<String, HttpError>) -> UploadOutcome {
    match result {
        Ok(stored_name) => UploadOutcome {
            succeeded: true,
            stored_name: Some(stored_name.clone()),
            message: format!("Uploaded {stored_name}"),
        },
        Err(error) => UploadOutcome {
            succeeded: false,
            stored_name: None,
            // A transport error without a message body (dioxus may answer
            // one) falls back to the generic copy.
            message: error
                .message
                .filter(|message| !message.is_empty())
                .unwrap_or_else(|| "Upload failed, try again later.".to_string()),
        },
    }
}

// `server` only: the fns under test are server fns and their fixtures use the
// server-only test support. Kept as two `cfg` attributes rather than
// `all(test, feature = "server")` so tooling recognises the test module.
#[cfg(test)]
#[cfg(feature = "server")]
mod tests {
    // The axum-test round trips below await !Send auto-futures; `#[sqlx::test]`
    // runs each body with block_on on one thread, so `Send` is not needed.
    #![expect(clippy::future_not_send)]

    use sqlx::PgPool;

    use super::*;
    use crate::models::artist::Artist;
    use crate::models::auth::User;
    use crate::models::release::Release;
    use crate::models::test_helpers::{
        create_test_artist, create_test_release, create_test_user_with_permissions,
    };
    use crate::routes::auth::Login;
    use crate::server::api::test_support::{api_server, api_server_with_uploads, post_args};
    use crate::services::files::progress::{FILES, add_chunk};

    fn login_args(username: &str) -> Login {
        Login {
            username: username.to_string(),
            password: "password".to_string(),
            remember: None,
        }
    }

    /// One upload in the dioxus protocol: the file bytes are the entire body,
    /// with the mime type as the content type and the name in a header.
    fn post_file(
        server: &axum_test::TestServer,
        config: &str,
        slug: &str,
        filename: &str,
        content_type: &str,
        bytes: &[u8],
    ) -> axum_test::TestRequest {
        server
            .post(&format!(
                "/api/upload?file_type={config}&slug={slug}&filename={filename}"
            ))
            .content_type(content_type)
            .add_header(
                "Content-Disposition",
                format!("attachment; filename=\"{filename}\""),
            )
            .bytes(axum::body::Bytes::copy_from_slice(bytes))
    }

    /// A temp uploads root with the directories `setup` normally creates.
    fn upload_dirs() -> (tempfile::TempDir, std::path::PathBuf) {
        let root = tempfile::tempdir().expect("upload dir");
        for subdirectory in ["tmp", "avatars", "artists", "releases"] {
            std::fs::create_dir_all(root.path().join(subdirectory))
                .expect("subdirectory should exist");
        }
        let path = root.path().to_path_buf();
        (root, path)
    }

    fn stored_names(dir: &std::path::Path) -> Vec<String> {
        std::fs::read_dir(dir)
            .expect("directory should be readable")
            .map(|entry| {
                entry
                    .expect("directory entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    }

    #[sqlx::test]
    async fn avatar_upload_is_stored_and_attached(pool: PgPool) {
        create_test_user_with_permissions(&pool, 1, vec![])
            .await
            .expect("plain user");
        let (_root, upload_root) = upload_dirs();
        let server = api_server_with_uploads(&pool, upload_root.clone()).await;
        post_args(&server, "/api/login", &login_args("test-1"))
            .await
            .assert_status_ok();

        let response = post_file(
            &server,
            "Avatar",
            "test-1",
            "holiday.png",
            "image/png",
            b"png bytes",
        )
        .await;

        response.assert_status_ok();
        // The server fn answers the name the file was stored under.
        let stored_name: String = response.json();
        assert!(
            stored_name.ends_with("-test-1.png"),
            "avatars are named after the uploader: {stored_name}"
        );
        let avatars = upload_root.join("avatars");
        assert_eq!(stored_names(&avatars), vec![stored_name.clone()]);
        assert_eq!(
            std::fs::read(avatars.join(&stored_name)).expect("stored bytes"),
            b"png bytes"
        );
        assert_eq!(
            stored_names(&upload_root.join("tmp")),
            Vec::<String>::new(),
            "the temp file is moved, not left behind"
        );

        let user = User::get_by_username(&pool, "test-1".to_string())
            .await
            .expect("user after upload");
        assert_eq!(user.avatar.as_deref(), Some(stored_name.as_str()));
    }

    /// Attaching an uploaded image must leave the artist's own columns alone.
    /// `Artist::update` re-derives `slug` from `name`, so the whole-model save
    /// this tail used to do silently renamed every artist whose slug did not
    /// derive from its name (the end-to-end fixture lost its `e2e-tabs` slug
    /// this way, which broke the images page behind it).
    #[sqlx::test]
    async fn artist_image_upload_does_not_rewrite_the_slug(pool: PgPool) {
        let artist = create_test_artist(&pool, 6, None)
            .await
            .expect("test artist");
        // A slug `slugify("Test Artist 6")` would never produce.
        sqlx::query("UPDATE artists SET slug = $1 WHERE id = $2")
            .bind("jimi-hendrix")
            .bind(artist.id)
            .execute(&pool)
            .await
            .expect("hand-set artist slug");
        create_test_user_with_permissions(&pool, 6, vec!["admin", "label_owner"])
            .await
            .expect("admin user");

        let (_root, upload_root) = upload_dirs();
        let server = api_server_with_uploads(&pool, upload_root.clone()).await;
        post_args(&server, "/api/login", &login_args("test-6"))
            .await
            .assert_status_ok();

        let response = post_file(
            &server,
            "Artist",
            "jimi-hendrix",
            "live.png",
            "image/png",
            b"png bytes",
        )
        .await;

        response.assert_status_ok();
        let stored_name: String = response.json();
        assert_eq!(
            stored_names(&upload_root.join("artists")),
            vec![stored_name.clone()],
            "the file lands in the artists directory"
        );

        let after_upload = Artist::get_by_slug(&pool, "jimi-hendrix".to_string())
            .await
            .expect("the artist is still reachable by its own slug");
        assert_eq!(after_upload.id, artist.id);
        assert_eq!(after_upload.name, "Test Artist 6");
        assert_eq!(
            after_upload.primary_image.as_deref(),
            Some(stored_name.as_str()),
            "the upload still attaches the image"
        );
        assert!(
            Artist::find_by_slug(&pool, "test-artist-6".to_string())
                .await
                .expect("lookup by the slug the model would derive")
                .is_none(),
            "attaching an image must not derive a slug from the artist's name"
        );
    }

    /// The release image path has the same guard: `Release::update` slugifies
    /// the name too, so a hand-set release slug must survive an upload.
    #[sqlx::test]
    async fn release_image_upload_does_not_rewrite_the_slug(pool: PgPool) {
        let artist = create_test_artist(&pool, 7, None)
            .await
            .expect("test artist");
        let release = create_test_release(&pool, 7, Some(artist.clone()))
            .await
            .expect("test release");
        sqlx::query("UPDATE releases SET slug = $1 WHERE id = $2")
            .bind("areta-now")
            .bind(release.id)
            .execute(&pool)
            .await
            .expect("hand-set release slug");
        create_test_user_with_permissions(&pool, 7, vec!["admin", "label_owner"])
            .await
            .expect("admin user");

        let (_root, upload_root) = upload_dirs();
        let server = api_server_with_uploads(&pool, upload_root.clone()).await;
        post_args(&server, "/api/login", &login_args("test-7"))
            .await
            .assert_status_ok();

        let response = post_file(
            &server,
            "Release",
            "areta-now",
            "cover.png",
            "image/jpeg",
            b"jpeg bytes",
        )
        .await;

        response.assert_status_ok();
        let stored_name: String = response.json();
        assert_eq!(
            stored_names(&upload_root.join("releases")),
            vec![stored_name.clone()],
            "the file lands in the releases directory"
        );

        let after_upload = Release::get_by_slug(&pool, "areta-now".to_string())
            .await
            .expect("the release is still reachable by its own slug");
        assert_eq!(after_upload.id, release.id);
        assert_eq!(after_upload.name, "Test Release 7");
        assert_eq!(
            after_upload.primary_image.as_deref(),
            Some(stored_name.as_str()),
            "the upload still attaches the image"
        );
        assert!(
            Release::find_by_slug(&pool, "test-release-7".to_string())
                .await
                .expect("lookup by the slug the model would derive")
                .is_none(),
            "attaching an image must not derive a slug from the release's name"
        );
    }

    #[sqlx::test]
    async fn upload_without_a_session_is_unauthorized(pool: PgPool) {
        let (_root, upload_root) = upload_dirs();
        let server = api_server_with_uploads(&pool, upload_root).await;

        let response = post_file(
            &server,
            "Avatar",
            "test-1",
            "holiday.png",
            "image/png",
            b"png bytes",
        )
        .await;

        response.assert_status_unauthorized();
    }

    #[sqlx::test]
    async fn upload_rejects_a_disallowed_mime_type(pool: PgPool) {
        create_test_user_with_permissions(&pool, 2, vec![])
            .await
            .expect("plain user");
        let (_root, upload_root) = upload_dirs();
        let server = api_server_with_uploads(&pool, upload_root.clone()).await;
        post_args(&server, "/api/login", &login_args("test-2"))
            .await
            .assert_status_ok();

        let response = post_file(
            &server,
            "Avatar",
            "test-2",
            "notes.txt",
            "text/plain",
            b"hi",
        )
        .await;

        response.assert_status(axum::http::StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(
            stored_names(&upload_root.join("avatars")),
            Vec::<String>::new(),
            "a rejected upload stores nothing"
        );
    }

    /// The size limit is enforced while streaming, not after buffering.
    #[sqlx::test]
    async fn upload_enforces_the_size_limit(pool: PgPool) {
        create_test_user_with_permissions(&pool, 3, vec![])
            .await
            .expect("plain user");
        let (_root, upload_root) = upload_dirs();
        let server = api_server_with_uploads(&pool, upload_root.clone()).await;
        post_args(&server, "/api/login", &login_args("test-3"))
            .await
            .assert_status_ok();

        // `Avatar` allows 10 MiB.
        let oversized = vec![b'x'; 10 * 1024 * 1024 + 1];
        let response = post_file(
            &server,
            "Avatar",
            "test-3",
            "big.png",
            "image/png",
            &oversized,
        )
        .await;

        response.assert_status(axum::http::StatusCode::PAYLOAD_TOO_LARGE);
        assert!(
            response.text().contains("File too large"),
            "the limit is explained: {}",
            response.text()
        );
        assert_eq!(
            stored_names(&upload_root.join("avatars")),
            Vec::<String>::new(),
            "an oversized upload stores nothing"
        );
        assert_eq!(
            stored_names(&upload_root.join("tmp")),
            Vec::<String>::new(),
            "the partial file is undone"
        );
    }

    /// An artist image needs `admin` + `label_owner`; a plain session may
    /// still upload an avatar (the config's own permission list).
    #[sqlx::test]
    async fn upload_needs_the_configs_permissions(pool: PgPool) {
        create_test_user_with_permissions(&pool, 4, vec![])
            .await
            .expect("plain user");
        let (_root, upload_root) = upload_dirs();
        let server = api_server_with_uploads(&pool, upload_root.clone()).await;
        post_args(&server, "/api/login", &login_args("test-4"))
            .await
            .assert_status_ok();

        let response = post_file(
            &server,
            "Artist",
            "test-artist-4",
            "cover.png",
            "image/png",
            b"png bytes",
        )
        .await;

        response.assert_status_forbidden();
        assert_eq!(
            stored_names(&upload_root.join("tmp")),
            Vec::<String>::new(),
            "a forbidden upload never reaches the temp dir"
        );
    }

    #[sqlx::test]
    async fn progress_without_a_session_is_unauthorized(pool: PgPool) {
        let server = api_server(pool).await;

        server
            .get("/api/file_progress?filename=watched.png")
            .await
            .assert_status_unauthorized();
    }

    /// Progress totals go to the requester's own upload. The broadcast
    /// channel replays totals recorded before the stream subscribed (the
    /// widget opens the stream before the upload starts), then live totals,
    /// and ends when the upload finishes (which drops the registry entry).
    /// The widget's parser keeps only the last total, so seeing both is fine.
    #[sqlx::test]
    async fn progress_totals_reach_the_requester(pool: PgPool) {
        create_test_user_with_permissions(&pool, 5, vec![])
            .await
            .expect("plain user");
        let server = api_server(pool).await;
        post_args(&server, "/api/login", &login_args("test-5"))
            .await
            .assert_status_ok();

        // The entry exists before the stream opens, as the widget arranges.
        add_chunk("watched.png", 5, "test-5").await;
        let publisher = tokio::spawn(async {
            // Give the handler a moment to subscribe before publishing.
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            add_chunk("watched.png", 3, "test-5").await;
            FILES.remove("test-5-watched.png");
        });

        let response = server.get("/api/file_progress?filename=watched.png").await;

        response.assert_status_ok();
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::CONTENT_TYPE)
                .expect("content type")
                .to_str()
                .expect("ascii"),
            "text/plain; charset=utf-8"
        );
        assert_eq!(response.text(), "5\n8\n");
        publisher.await.expect("publisher task");
    }

    #[test]
    fn upload_outcome_messages() {
        assert_eq!(
            upload_outcome(Ok("someone.png".to_string())).message,
            "Uploaded someone.png"
        );
        assert!(upload_outcome(Ok("someone.png".to_string())).succeeded);
        // The stored name travels with the success so pages can build its URL.
        assert_eq!(
            upload_outcome(Ok("someone.png".to_string()))
                .stored_name
                .as_deref(),
            Some("someone.png")
        );
        assert_eq!(
            upload_outcome(HttpError::payload_too_large(
                "File too large (the limit is 10 MB)."
            ))
            .message,
            "File too large (the limit is 10 MB)."
        );
        // A dioxus transport error with no body keeps the generic copy.
        let bare = HttpError::new(axum::http::StatusCode::BAD_GATEWAY, "");
        let failure = upload_outcome(Err(bare));
        assert_eq!(failure.message, "Upload failed, try again later.");
        assert!(failure.stored_name.is_none());
    }

    #[test]
    fn stored_files_are_urls_under_their_config_directory() {
        // Mirrors `Artist::primary_image_url`/`Release::primary_image_url`
        // and the avatar path, so an upload swaps straight into an `<img>`.
        assert_eq!(
            stored_file_url(UploadConfiguration::Artist, "123-jimi.png"),
            "/uploads/artists/123-jimi.png"
        );
        assert_eq!(
            stored_file_url(UploadConfiguration::Release, "123-axis.jpg"),
            "/uploads/releases/123-axis.jpg"
        );
        assert_eq!(
            stored_file_url(UploadConfiguration::Avatar, "123-test-1.png"),
            "/uploads/avatars/123-test-1.png"
        );
    }

    /// The widget renders its controls on the server, including the
    /// config-driven `accept` list.
    #[test]
    fn widget_renders_the_file_controls() {
        let html = dioxus::ssr::render_element(rsx! {
            FileUploadWithProgress {
                config: UploadConfiguration::Avatar,
                slug: "someone",
            }
        });
        assert!(
            html.contains(r#"name="file_to_upload""#),
            "the picker keeps the Leptos field name: {html}"
        );
        assert!(
            html.contains(r#"class="w-full file-input file-input-primary""#),
            "daisy file input: {html}"
        );
        assert!(
            html.contains(r#"accept="image/jpeg,image/png,image/gif,image/webp""#),
            "the accept list comes from the config: {html}"
        );
        assert!(html.contains("Upload"), "the button says Upload: {html}");
    }
}
