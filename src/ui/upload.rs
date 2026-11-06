//! The upload widget and its endpoints.
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
//! Design notes:
//! - No plain-HTML *body* twin: `/api/upload` speaks dioxus' body protocol, so
//!   the reactive path needs the client build. The widget also renders a real
//!   `multipart/form-data` form, so a browser with javascript off posts to
//!   `/forms/upload` instead (`crate::server::upload_form` answers it); the two
//!   paths share the filename rules, the signature check and the
//!   `finalise_upload` tail.
//! - The type a file claims to be is worked out from its extension when the
//!   request's own `Content-Type` is not one of the accepted types, because a
//!   dioxus window cannot put the file's own type in that header. Both upload
//!   paths then check the first bytes of what arrived, so a wrong claim is only
//!   ever a wrong guess about the message the user sees.
//! - [`file_progress`](crate::ui::upload::file_progress) needs only a session,
//!   not `admin` + `label_owner`: the
//!   registry key already carries the requester's username, so the extra
//!   check did nothing except hide the avatar progress bar behind a 403 for
//!   everyone without those permissions.
//! - [`upload_file`](crate::ui::upload::upload_file) answers the stored file
//!   name, which the widget shows.

use dioxus::fullstack::{FileStream, TextStream};
use dioxus::html::FileData;
use dioxus::prelude::*;

use crate::config::upload::UploadConfiguration;
#[cfg(feature = "server")]
use crate::config::upload::UploadDetails;
use crate::ui::assets::served_file_url;
use crate::ui::utils::{InlineError, Success};
use crate::utils::files::{accept_extensions, file_extension, image_mime_for_extension};

#[cfg(feature = "server")]
use crate::services::authentication_helpers::user_with_permissions;
#[cfg(feature = "server")]
use crate::services::files::progress::{FILES, add_chunk, progress_for_file};
#[cfg(feature = "server")]
use crate::services::files::upload::{UploadedFile, finalise_upload};
#[cfg(feature = "server")]
use crate::ui::data::to_http_error;
#[cfg(feature = "server")]
use crate::utils::files::{SIGNATURE_BYTES, first_bytes, image_mime_in_bytes, valid_file_name};
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
/// The bytes stream to `<root>/tmp` with [`stream_to_tmp`], their first bytes
/// have to match one of the configuration's image types
/// ([`check_upload_signature`]), and only then does the shared
/// `services::files::upload` tail move them into the config's directory and
/// update the row — the same tail the no-JS form handler in
/// `server::upload_form` uses, so storage and
/// attachment cannot drift.
///
/// What the client *claims* the file is (see [`claimed_upload_type`]) is only a
/// fast rejection before the body arrives; the bytes and the declared byte count
/// decide what is kept.
///
/// # Errors
/// 400 for an unknown configuration, a rejected filename or a body that is not
/// the length the client declared, 401 with no session, 403 when the
/// configuration needs permissions the session lacks, 415 for a claimed type the
/// configuration disallows or bytes that are not one of its image types, 413 once
/// the stream passes the size limit, 500 when the uploads root is missing or the
/// write fails.
#[post("/api/upload?file_type&slug&filename",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser,
    upload_root: axum::Extension<crate::server::UploadRoot>
)]
pub async fn upload_file(
    file_type: String,
    slug: String,
    filename: String,
    file_stream: FileStream,
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

    let header_type = file_stream.content_type().unwrap_or_default().to_string();
    // Read before the body is handed over to the writer.
    let declared_size = file_stream.size();
    let claimed_type = claimed_upload_type(&header_type, &filename, &details.mime_types);
    if !details
        .mime_types
        .iter()
        .any(|allowed| allowed == claimed_type)
    {
        tracing::warn!("upload {filename} claims {claimed_type}, which {file_type} does not allow");
        return HttpError::unsupported_media_type("Invalid mime type.");
    }
    // `filename` is the client's own copy (see the module docs); the
    // Content-Disposition name would arrive percent-encoded. The last
    // argument keeps the overwrite-an-existing-file behaviour.
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
    // The progress registry key this upload is watched by: the client sends the
    // filename itself rather than relying on the `Content-Disposition` copy.
    let progress_key = format!("{}-{filename}", uploader.username);
    // Undoing a rejected upload is the same two steps every time: take the
    // partial file away, and let any progress stream watching it end.
    let undo_upload = || {
        if let Err(error) = std::fs::remove_file(&tmp_path) {
            tracing::error!("could not remove the partial upload {tmp_path}: {error}");
        }
        FILES.remove(progress_key.as_str());
    };

    let received = match stream_to_tmp(
        file_stream,
        &tmp_path,
        details.size_limit,
        &filename,
        &uploader.username,
    )
    .await
    {
        Ok(received) => received,
        Err(error) => {
            undo_upload();
            return Err(error);
        }
    };

    if let Err(error) = check_upload_signature(&tmp_path, &filename, claimed_type, &details) {
        undo_upload();
        return Err(error);
    }

    // A body that stops early would otherwise be stored and attached as if it
    // were the whole file. Dioxus clients declare the length in
    // `X-Content-Size` (a browser overwrites `Content-Length`); the window path
    // gets the same guarantee from hyper, which enforces its `Content-Length`.
    if let Some(declared_size) = declared_size
        && declared_size != received as u64
    {
        tracing::warn!("upload {filename} sent {received} of the declared {declared_size} bytes");
        undo_upload();
        return HttpError::bad_request("The upload did not finish.");
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

/// The type an upload claims to be.
///
/// A browser puts the file's own type in `Content-Type`. A dioxus window puts
/// one dioxus invented from a table of *web asset* types that holds no images at
/// all (a `.png` comes back as `text/html; charset=utf-8`), so a header that is
/// not one of the `accepted` types falls back to the type the file's extension
/// stands for, and failing that to the header itself, so a rejection names what
/// the client actually sent.
///
/// This only decides whether a body is worth streaming at all;
/// [`check_upload_signature`] decides what the file is.
#[cfg(feature = "server")]
fn claimed_upload_type<'a>(content_type: &'a str, file_name: &str, accepted: &[String]) -> &'a str {
    if accepted.iter().any(|allowed| allowed == content_type) {
        return content_type;
    }
    file_extension(file_name)
        .and_then(image_mime_for_extension)
        .unwrap_or(content_type)
}

/// Streams the request body to `tmp_path`, counting each chunk into the progress
/// registry and refusing to pass `size_limit`; answers how many bytes arrived.
///
/// # Errors
/// 413 once the stream passes the limit, 400 when the stream itself fails, 500
/// when the temp file cannot be created or written.
#[cfg(feature = "server")]
async fn stream_to_tmp(
    mut file_stream: FileStream,
    tmp_path: &str,
    size_limit: usize,
    original_file_name: &str,
    username: &str,
) -> core::result::Result<usize, HttpError> {
    let mut tmp_file = std::fs::File::create(tmp_path).map_err(|error| {
        tracing::error!("cannot create {tmp_path}: {error}");
        HttpError::new(
            http::StatusCode::INTERNAL_SERVER_ERROR,
            "Upload write failed.",
        )
    })?;
    let mut received: usize = 0;
    while let Some(chunk) = file_stream.next().await {
        let chunk = chunk.map_err(|error| {
            tracing::error!("upload stream for {original_file_name} failed: {error}");
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
        add_chunk(original_file_name, chunk.len(), username).await;
    }
    Ok(received)
}

/// Checks what an uploaded file's bytes really are, before anything points at
/// them.
///
/// `claimed` only reaches the log: claim and bytes disagree most often when a
/// file was saved as one type and renamed as another, which is harmless while
/// the bytes are an accepted type.
///
/// # Errors
/// 415 when the bytes are not a recognised image, or are one the configuration
/// does not allow; 500 when the temp file cannot be read back.
#[cfg(feature = "server")]
fn check_upload_signature(
    tmp_path: &str,
    original_file_name: &str,
    claimed: &str,
    details: &UploadDetails,
) -> core::result::Result<(), HttpError> {
    let signature = first_bytes(tmp_path, SIGNATURE_BYTES).map_err(|error| {
        tracing::error!("cannot read back {tmp_path}: {error}");
        HttpError::new(
            http::StatusCode::INTERNAL_SERVER_ERROR,
            "Upload write failed.",
        )
    })?;
    match image_mime_in_bytes(&signature) {
        Some(actual) if actual == claimed => {}
        Some(actual) if allowed_image_type(actual, details) => {
            tracing::warn!("upload {original_file_name} was claimed as {claimed} but is {actual}");
        }
        Some(actual) => {
            tracing::warn!(
                "upload {original_file_name} is {actual}, which its configuration does not allow"
            );
            return HttpError::unsupported_media_type("That image type is not allowed here.");
        }
        None => {
            tracing::warn!(
                "upload {original_file_name} (claimed as {claimed}) is not a recognised image"
            );
            return HttpError::unsupported_media_type(
                "The file is not one of the accepted image types.",
            );
        }
    }
    Ok(())
}

/// Whether `mime_type` is one of the types the configuration accepts.
#[cfg(feature = "server")]
fn allowed_image_type(mime_type: &str, details: &UploadDetails) -> bool {
    details
        .mime_types
        .iter()
        .any(|allowed| allowed == mime_type)
}

/// The running byte totals of the caller's own in-flight upload, one per
/// line, until that upload finishes. The stream also ends if the upload never
/// arrives or stalls part way through, so a client can leave it without holding
/// a request open ([`crate::services::files::progress`] owns those waits).
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

/// File picker, Upload button and progress bar.
///
/// Two submission paths share one markup. The reactive one (`onsubmit`, so
/// `prevent_default`) streams the file through the server fn and draws the
/// progress bar. The javascript-off one posts the same form as
/// `multipart/form-data` to `server::upload_form`, which 303s back to this page
/// with `?uploaded=1` or `?bad_upload=<code>`; those codes come in as props and
/// render through [`upload_message`].
#[component]
pub fn FileUploadWithProgress(
    /// Which slot to upload to: chooses the directory, mime types, size limit
    /// and required permissions (see [`UploadConfiguration::get_details`]).
    config: UploadConfiguration,
    /// The item the file belongs to: an artist or release slug, or the
    /// uploader's own username for [`UploadConfiguration::Avatar`].
    slug: String,
    /// The artist slug, needed only by the javascript-off form when uploading
    /// for a release: its editor lives under the artist.
    #[props(optional)]
    artist_slug: Option<String>,
    /// This page's `?uploaded=1` code, set by a javascript-off upload.
    #[props(optional)]
    uploaded: Option<String>,
    /// This page's `?bad_upload=<code>` code, set by a failed javascript-off
    /// upload.
    #[props(optional)]
    bad_upload: Option<String>,
    /// Runs after a file is stored, with the name it was stored under. The
    /// artist and release pages use it to point their `<img>` at the new
    /// file, so the user does not have to reload.
    #[props(optional)]
    on_uploaded: Option<EventHandler<String>>,
) -> Element {
    let details = config.get_details();
    // Extension tokens rather than the config's mime types: a browser accepts
    // either, but the file dialog in a window understands only extensions, and a
    // mime type there leaves it offering every file on disk.
    let accepted_types = accept_extensions(&details.mime_types);
    let size_limit_mb = details.size_limit / (1024 * 1024);
    // The javascript-off form has to name the row it belongs to, and the slug
    // is moved into the action's future below, so keep a copy.
    let form_slug = slug.clone();
    let form_artist_slug = artist_slug;
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
    // The query-code alerts stand in for the outcome when there is no reactive
    // one, so decide that before `outcome` is moved into the markup below.
    let no_reactive_outcome = outcome.is_none();

    rsx! {
        form {
            // A file input needs a POST + multipart target even when javascript
            // runs: without one the browser refuses to send the file (and says
            // so in the console). `onsubmit` below prevents the post for the
            // reactive path, and `server::upload_form` answers it for the rest.
            method: "post",
            action: "/forms/upload",
            enctype: "multipart/form-data",
            onsubmit: move |evt| {
                // The upload protocol is a request body, not a form post.
                evt.prevent_default();
                sent_bytes.set(None);
                if let Some(file) = picked_file(&evt) {
                    uploading.set(Some(file.name()));
                    max_bytes.set(Some(file.size()));
                    // The progress bar is fed on its own task so the body can
                    // go up straight away (see [`watch_progress`]).
                    spawn(watch_progress(file.name(), sent_bytes, file.size()));
                    submit.call(file);
                }
            },
            // The javascript-off path needs them: `file_type` picks the
            // configuration, `slug` (and `artist_slug`) the row and the page to
            // come back to. Order is the protocol — the file part comes last.
            input {
                r#type: "hidden",
                name: "file_type",
                value: format!("{config}"),
            }
            input {
                r#type: "hidden",
                name: "slug",
                value: form_slug,
            }
            if let Some(artist) = form_artist_slug {
                input {
                    r#type: "hidden",
                    name: "artist_slug",
                    value: artist,
                }
            }
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
        // A javascript-off upload has no client state to hold its outcome, so
        // the handler carried it in the page's query codes instead. The
        // reactive path never sets those, so the two cannot both alert.
        if no_reactive_outcome {
            if uploaded.is_some() {
                Success { message: "Image uploaded.", show: true }
            }
            if let Some(message) = upload_message(bad_upload.as_deref()) {
                InlineError { message }
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

/// The on-screen message for the javascript-off upload form's fixed error
/// codes; unknown and absent codes render nothing. `server::upload_form` picks
/// the codes, mirroring how the other editors' `*_message` tables pair with the
/// handlers in `server::forms`.
fn upload_message(code: Option<&str>) -> Option<&'static str> {
    match code? {
        "forbidden" => Some("You do not have permission."),
        "not_found" => Some("The item to attach the file to was not found."),
        "bad_form" => Some("The upload form was incomplete."),
        "missing_file" => Some("Choose a file to upload."),
        "bad_mime" => Some("Invalid mime type."),
        "bad_image" => Some("The file is not one of the accepted image types."),
        "bad_file_name" => Some("Invalid file name."),
        "too_large" => Some("File too large."),
        "server" => Some("Upload failed."),
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
///
/// Absolute against the API origin in a desktop window, where the page's own
/// origin has no uploads at all (see [`crate::ui::assets`]).
#[must_use]
pub fn stored_file_url(config: UploadConfiguration, stored_name: &str) -> String {
    served_file_url(&format!(
        "/uploads/{}/{stored_name}",
        config.get_details().path
    ))
}

/// Runs one attempt: the cheap checks first, then the file body.
///
/// The progress bar is fed by [`watch_progress`], which the widget spawns
/// alongside this, rather than by a second request awaited here.
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

    // Cheap checks first, so an obviously doomed upload never sends its body.
    if size_bytes > u64::try_from(details.size_limit).unwrap_or(u64::MAX) {
        return Ok(upload_outcome(HttpError::payload_too_large(format!(
            "{filename} is larger than the {size_limit_mb} MB limit."
        ))));
    }
    let mime_type = client_declared_mime(&file);
    if !details.mime_types.contains(&mime_type) {
        return Ok(upload_outcome(HttpError::unsupported_media_type(format!(
            "{filename} is not a supported file type."
        ))));
    }

    let result = upload_file(config.to_string(), slug, filename, FileStream::from(file)).await;
    let outcome = upload_outcome(result);
    // The totals stream stops as soon as the file is fully up, so the last step
    // of the bar is the upload's own answer.
    if outcome.succeeded {
        sent_bytes.set(Some(size_bytes));
    }
    Ok(outcome)
}

/// The type the picker says the file is, for the server's `declared_mime`.
///
/// A browser answers with the file's own type. A dioxus window hands back a
/// path, and dioxus invents a type for it from a table of *web asset* types that
/// holds no images at all (a `.png` comes back as `text/html; charset=utf-8`),
/// so anything that is not an image is re-derived from the file's extension.
/// The server checks the bytes themselves, so a wrong guess here only ever
/// costs the user a rejection.
fn client_declared_mime(file: &FileData) -> String {
    declared_mime_from(&file.content_type().unwrap_or_default(), &file.name())
}

/// [`client_declared_mime`] with its two inputs in hand, so the quirk it works
/// around can be tested without a platform file object.
fn declared_mime_from(from_platform: &str, file_name: &str) -> String {
    if from_platform.starts_with("image/") {
        return from_platform.to_string();
    }
    // With no extension to go on, the request's own type is all there is.
    file_extension(file_name)
        .and_then(image_mime_for_extension)
        .unwrap_or(from_platform)
        .to_string()
}

/// Copies one upload's running byte totals into `sent_bytes` until that stream
/// ends.
///
/// This is a task of its own, spawned by the widget, for two reasons:
/// - A stream response with no bytes in it yet does not resolve in Firefox, so
///   awaiting it before the body goes up means the upload is never sent at all
///   (the totals wait for the upload, the upload waits for the totals).
/// - A request left open when the page navigates is aborted by the browser, and
///   dioxus turns an aborted request into a panic of the whole client. This one
///   always ends by itself: when its upload finishes, when the upload never
///   arrives, or when it stalls (`services::files::progress` owns those waits).
///
/// The last line of a chunk is the current total, because the HTTP layer may
/// pack several totals into one chunk.
#[expect(
    clippy::future_not_send,
    reason = "takes a dioxus Signal, which is !Send by design; this future only ever runs on the component's executor"
)]
async fn watch_progress(filename: String, mut sent_bytes: Signal<Option<u64>>, size_bytes: u64) {
    let mut totals = match file_progress(filename.clone()).await {
        Ok(totals) => totals,
        Err(error) => {
            tracing::error!("could not start the progress stream for {filename}: {error}");
            return;
        }
    };
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
                    if bytes >= size_bytes {
                        // The whole file is up; anything left in the stream is
                        // repeats, and the upload's answer finishes the bar.
                        break;
                    }
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
    use crate::server::test_support::{api_server, api_server_with_uploads, login_request};
    use crate::services::files::progress::{FILES, add_chunk};

    /// A real png signature followed by filler: the server checks the signature
    /// and stores the rest as it came, so tests that are not about image
    /// contents can keep the body short.
    const PNG_BODY: &[u8] = b"\x89PNG\r\n\x1a\npng body";
    const JPEG_BODY: &[u8] = b"\xff\xd8\xff\xe0jpeg body";

    /// One upload in the dioxus protocol, as a browser sends it: the file bytes
    /// are the entire body, the type arrives in `Content-Type`, the length is
    /// declared in `X-Content-Size`, and the name arrives in a header.
    fn post_file(
        server: &axum_test::TestServer,
        config: &str,
        slug: &str,
        filename: &str,
        content_type: &str,
        bytes: &[u8],
    ) -> axum_test::TestRequest {
        upload_request(
            server,
            config,
            slug,
            filename,
            content_type,
            Some(bytes.len()),
            bytes,
        )
    }

    /// The same request with the header and the declared length under the test's
    /// own control, since the interesting cases are the ones where they disagree
    /// with the bytes: the window path sends a `Content-Type` dioxus invented
    /// from the extension and declares no length (hyper enforces its
    /// `Content-Length` instead).
    fn upload_request(
        server: &axum_test::TestServer,
        config: &str,
        slug: &str,
        filename: &str,
        content_type: &str,
        declared_size: Option<usize>,
        bytes: &[u8],
    ) -> axum_test::TestRequest {
        let request = server.post(&format!(
            "/api/upload?file_type={config}&slug={slug}&filename={filename}"
        ));
        let request = match declared_size {
            Some(size) => request.add_header("X-Content-Size", size.to_string()),
            None => request,
        };
        request
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
        login_request(&server, "test-1").await.assert_status_ok();

        let response = post_file(
            &server,
            "Avatar",
            "test-1",
            "holiday.png",
            "image/png",
            PNG_BODY,
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
            PNG_BODY
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
        login_request(&server, "test-6").await.assert_status_ok();

        let response = post_file(
            &server,
            "Artist",
            "jimi-hendrix",
            "live.png",
            "image/png",
            PNG_BODY,
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
        login_request(&server, "test-7").await.assert_status_ok();

        let response = post_file(
            &server,
            "Release",
            "areta-now",
            "cover.jpg",
            "image/jpeg",
            JPEG_BODY,
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
            PNG_BODY,
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
        login_request(&server, "test-2").await.assert_status_ok();

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

    /// A `.png` name and an `image/png` claim cannot carry a text file: the
    /// first bytes decide, and the file is gone again before anything points at
    /// it.
    #[sqlx::test]
    async fn upload_rejects_bytes_that_are_not_an_image(pool: PgPool) {
        create_test_user_with_permissions(&pool, 8, vec![])
            .await
            .expect("plain user");
        let (_root, upload_root) = upload_dirs();
        let server = api_server_with_uploads(&pool, upload_root.clone()).await;
        login_request(&server, "test-8").await.assert_status_ok();

        let response = post_file(
            &server,
            "Avatar",
            "test-8",
            "portrait.png",
            "image/png",
            b"not an image at all",
        )
        .await;

        response.assert_status(axum::http::StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(
            stored_names(&upload_root.join("avatars")),
            Vec::<String>::new(),
            "a file with the wrong bytes stores nothing"
        );
        assert_eq!(
            stored_names(&upload_root.join("tmp")),
            Vec::<String>::new(),
            "the temp file is undone"
        );
        let user = User::get_by_username(&pool, "test-8".to_string())
            .await
            .expect("user after the rejected upload");
        assert_eq!(user.avatar, None, "and the row is left alone");
    }

    /// The window build sends the type it worked out from the extension in the
    /// argument, a `Content-Type` dioxus invented from a table of web asset
    /// types, and no length of its own. That is the shape that must still work.
    #[sqlx::test]
    async fn upload_accepts_the_window_builds_shape(pool: PgPool) {
        create_test_user_with_permissions(&pool, 9, vec![])
            .await
            .expect("plain user");
        let (_root, upload_root) = upload_dirs();
        let server = api_server_with_uploads(&pool, upload_root.clone()).await;
        login_request(&server, "test-9").await.assert_status_ok();

        let response = upload_request(
            &server,
            "Avatar",
            "test-9",
            "portrait.jpeg",
            "text/html; charset=utf-8",
            None,
            JPEG_BODY,
        )
        .await;

        response.assert_status_ok();
        let stored_name: String = response.json();
        assert_eq!(
            stored_names(&upload_root.join("avatars")),
            vec![stored_name],
            "the extension carries the upload past the header check"
        );
    }

    /// A body that stops short of the declared length must not be stored and
    /// attached as if it were the whole file.
    #[sqlx::test]
    async fn upload_rejects_a_body_shorter_than_declared(pool: PgPool) {
        create_test_user_with_permissions(&pool, 10, vec![])
            .await
            .expect("plain user");
        let (_root, upload_root) = upload_dirs();
        let server = api_server_with_uploads(&pool, upload_root.clone()).await;
        login_request(&server, "test-10").await.assert_status_ok();

        let response = upload_request(
            &server,
            "Avatar",
            "test-10",
            "cut-off.png",
            "image/png",
            Some(PNG_BODY.len() + 100),
            PNG_BODY,
        )
        .await;

        response.assert_status_bad_request();
        assert!(
            response.text().contains("did not finish"),
            "the reason is explained: {}",
            response.text()
        );
        assert_eq!(
            stored_names(&upload_root.join("avatars")),
            Vec::<String>::new(),
            "a truncated upload stores nothing"
        );
        assert_eq!(
            stored_names(&upload_root.join("tmp")),
            Vec::<String>::new(),
            "the partial file is undone"
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
        login_request(&server, "test-3").await.assert_status_ok();

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
        login_request(&server, "test-4").await.assert_status_ok();

        let response = post_file(
            &server,
            "Artist",
            "test-artist-4",
            "cover.png",
            "image/png",
            PNG_BODY,
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
        login_request(&server, "test-5").await.assert_status_ok();

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

    /// The window build asks dioxus for a file's type, and dioxus answers from
    /// a table of web asset types that has no images in it, so a `.png` comes
    /// back claiming to be html. The extension has to win.
    #[test]
    fn the_window_builds_invented_type_gives_way_to_the_extension() {
        assert_eq!(
            declared_mime_from("text/html; charset=utf-8", "portrait.png"),
            "image/png"
        );
        assert_eq!(
            declared_mime_from("application/octet-stream", "portrait.JPG"),
            "image/jpeg"
        );
        // A browser's own answer is already right, and stays untouched.
        assert_eq!(declared_mime_from("image/webp", "cover.webp"), "image/webp");
        // Nothing to work with: the request's own type is all there is.
        assert_eq!(
            declared_mime_from("application/octet-stream", "no-extension"),
            "application/octet-stream"
        );
        assert_eq!(
            declared_mime_from("text/html; charset=utf-8", "notes.txt"),
            "text/html; charset=utf-8"
        );
    }

    /// The window build sends a `Content-Type` dioxus invented from a table of
    /// web asset types, so a header that is not one of the accepted types has to
    /// fall back to the file's extension.
    #[test]
    fn a_claim_the_window_invented_falls_back_to_the_extension() {
        let accepted = UploadConfiguration::Avatar.get_details().mime_types;
        // A browser's own type is already one of them, so it stands.
        assert_eq!(
            claimed_upload_type("image/gif", "animation.gif", &accepted),
            "image/gif"
        );
        assert_eq!(
            claimed_upload_type("text/html; charset=utf-8", "cover.PNG", &accepted),
            "image/png"
        );
        assert_eq!(
            claimed_upload_type("application/octet-stream", "cover.webp", &accepted),
            "image/webp"
        );
        // Nothing to fall back to: the claim itself is what gets rejected.
        assert_eq!(
            claimed_upload_type("text/plain", "notes.txt", &accepted),
            "text/plain"
        );
        assert_eq!(
            claimed_upload_type("text/html; charset=utf-8", "no-extension", &accepted),
            "text/html; charset=utf-8"
        );
    }

    #[test]
    fn the_signature_check_decides_what_is_stored() {
        let details = UploadConfiguration::Avatar.get_details();

        let real_png = tempfile::NamedTempFile::new().expect("temp file");
        std::fs::write(real_png.path(), PNG_BODY).expect("write");
        check_upload_signature(
            &real_png.path().to_string_lossy(),
            "portrait.png",
            "image/png",
            &details,
        )
        .expect("a png is accepted");

        // Renamed, but still a real png: stored, with a note in the log.
        check_upload_signature(
            &real_png.path().to_string_lossy(),
            "portrait.jpg",
            "image/jpeg",
            &details,
        )
        .expect("the bytes decide, and they are a png");

        let text_file = tempfile::NamedTempFile::new().expect("temp file");
        std::fs::write(text_file.path(), b"a letter to the label").expect("write");
        let rejected = check_upload_signature(
            &text_file.path().to_string_lossy(),
            "portrait.png",
            "image/png",
            &details,
        )
        .expect_err("a text file is not an image");
        assert_eq!(
            rejected.status,
            axum::http::StatusCode::UNSUPPORTED_MEDIA_TYPE
        );

        // A file too short to hold a signature cannot be one either.
        let empty = tempfile::NamedTempFile::new().expect("temp file");
        assert!(
            check_upload_signature(
                &empty.path().to_string_lossy(),
                "nothing.png",
                "image/png",
                &details,
            )
            .is_err(),
            "an empty file is refused"
        );

        // A missing temp file is a write failure, not a mime problem.
        let unreadable =
            check_upload_signature("/no/such/upload.png", "upload.png", "image/png", &details)
                .expect_err("a missing file cannot be checked");
        assert_eq!(
            unreadable.status,
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
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
            "the picker's field is named file_to_upload: {html}"
        );
        assert!(
            html.contains(r#"class="w-full file-input file-input-primary""#),
            "daisy file input: {html}"
        );
        assert!(
            html.contains(r#"accept=".jpg,.JPG,.jpeg,.JPEG,.png,.PNG,.gif,.GIF,.webp,.WEBP""#),
            "the accept list comes from the config, as extension tokens a \
             window's file dialog can also read: {html}"
        );
        assert!(html.contains("Upload"), "the button says Upload: {html}");
    }

    /// The same markup has to be a working form without javascript: a POST to
    /// the upload handler, multipart encoding, and the hidden fields the
    /// handler needs (a file input in a form that posts none of that is
    /// dropped by the browser, with a console warning).
    #[test]
    fn widget_renders_a_form_that_works_without_javascript() {
        let html = dioxus::ssr::render_element(rsx! {
            FileUploadWithProgress {
                config: UploadConfiguration::Release,
                slug: "test-release-1",
                artist_slug: Some("test-artist-1".to_string()),
            }
        });
        for expected in [
            r#"method="post""#,
            r#"enctype="multipart/form-data""#,
            r#"action="/forms/upload""#,
            r#"name="file_type""#,
            r#"value="test-release-1""#,
            r#"name="artist_slug""#,
            r#"value="test-artist-1""#,
        ] {
            assert!(html.contains(expected), "expected {expected} in: {html}");
        }
        assert!(
            html.contains(r#"name="file_type" value="Release""#),
            "the configuration travels as a hidden field: {html}"
        );
        // The markup's action and the router's route are the same path, and
        // that only holds while this pin matches `server::upload_form`.
        assert_eq!(
            crate::server::upload_form::PATH_UPLOAD_FORM,
            "/forms/upload",
            "the form action above is the handler's route"
        );
    }

    #[test]
    fn upload_messages_cover_the_known_codes() {
        assert_eq!(
            upload_message(Some("forbidden")),
            Some("You do not have permission.")
        );
        assert_eq!(
            upload_message(Some("missing_file")),
            Some("Choose a file to upload.")
        );
        assert_eq!(upload_message(Some("bad_mime")), Some("Invalid mime type."));
        assert_eq!(
            upload_message(Some("bad_image")),
            Some("The file is not one of the accepted image types.")
        );
        assert_eq!(upload_message(Some("too_large")), Some("File too large."));
        assert_eq!(upload_message(Some("server")), Some("Upload failed."));
        assert_eq!(upload_message(Some("made_up")), None);
        assert_eq!(upload_message(None), None);
    }
}
