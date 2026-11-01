//! Server functions: the Dioxus equivalent of the main app's `#[server]`
//! endpoints. Paths and payloads mirror the Leptos app's `/api` endpoints
//! where the spike needs them.
//!
//! Extractor-only arguments (`auth`, `db`) are listed in the macro attribute
//! rather than the function signature: the server injects them, the client
//! never sees them (pattern from Dioxus' `07-fullstack/auth` example).
//!
//! Error convention: every server fn declares `Result<T, HttpError>`
//! explicitly instead of the prelude's `Result<T>` (= `CapturedError`).
//! `HttpError::xxx(..)` constructors return a ready-made `Result<T, HttpError>`
//! for any `T`, so bailing out is just `return HttpError::bad_request("...");`.
//! Verified at runtime: with `HttpError` the wire response carries the real
//! HTTP status; with the prelude `Result` every server-fn error arrives as
//! HTTP 500 with the intended code only inside the JSON body.

#[cfg(feature = "server")]
use dioxus::fullstack::Streaming;
use dioxus::fullstack::{FileStream, TextStream};
use dioxus::prelude::*;
#[cfg(feature = "server")]
use futures::StreamExt;

use crate::models::{Artist, User};

/// The current session user, or `None` (main app: `GetUser` at /api/user).
#[get("/api/user", auth: crate::auth::Session)]
pub async fn get_user() -> core::result::Result<Option<User>, HttpError> {
    Ok(auth.current_user)
}

/// Login with username and password, starting the session (main app: `Login`).
#[post("/api/user/login", auth: crate::auth::Session, db: axum::Extension<crate::db::Db>)]
pub async fn login(
    username: String,
    password: String,
    remember: Option<String>,
) -> core::result::Result<User, HttpError> {
    if username.is_empty() || password.is_empty() {
        return HttpError::bad_request("Username and password are required.");
    }

    let stored = match sqlx::query_as::<_, (i64, String)>(
        "SELECT id, password FROM users WHERE username = $1",
    )
    .bind(&username)
    .fetch_optional(&db.0.0)
    .await
    {
        Ok(stored) => stored,
        Err(error) => {
            tracing::error!("login: database error: {error}");
            return HttpError::internal_server_error("Login failed");
        }
    };

    let Some((user_id, passhash)) = stored else {
        return HttpError::unauthorized("User does not exist.");
    };

    let password_ok = match bcrypt::verify(password, &passhash) {
        Ok(password_ok) => password_ok,
        Err(error) => {
            tracing::error!("login: bcrypt error: {error}");
            return HttpError::internal_server_error("Login failed");
        }
    };
    if !password_ok {
        return HttpError::unauthorized("Password does not match.");
    }

    auth.login_user(user_id);
    auth.remember_user(remember.is_some());

    // Verified behaviour of axum_session_auth 0.20.1: `login_user` writes the
    // session but does NOT populate `current_user` in the same request (the
    // cookie works from the next request on). So return the user loaded
    // explicitly instead of reading `auth.current_user`.
    match <User as axum_session_auth::Authentication<User, i64, sqlx::PgPool>>::load_user(
        user_id,
        Some(&db.0.0),
    )
    .await
    {
        Ok(user) => Ok(user),
        Err(error) => {
            tracing::error!("login: session set but load_user failed: {error}");
            HttpError::internal_server_error("Login succeeded but the user could not be loaded")
        }
    }
}

/// End the session (main app: `Logout`).
#[post("/api/user/logout", auth: crate::auth::Session)]
pub async fn logout() -> core::result::Result<Option<User>, HttpError> {
    auth.logout_user();
    Ok(None)
}

/// Artists listing used by the `/artists` page: gives the spike a way to
/// discover real slugs through SSR output (see also `get_artist`).
#[get("/api/artists", db: axum::Extension<crate::db::Db>)]
pub async fn list_artists() -> core::result::Result<Vec<Artist>, HttpError> {
    #[derive(sqlx::FromRow)]
    struct ArtistRow {
        id: i64,
        name: String,
        slug: String,
        description: Option<String>,
    }

    let rows = match sqlx::query_as::<_, ArtistRow>(
        "SELECT id, name, slug, description FROM artists ORDER BY name LIMIT 10",
    )
    .fetch_all(&db.0.0)
    .await
    {
        Ok(rows) => rows,
        Err(error) => {
            tracing::error!("artists list: database error: {error}");
            return HttpError::internal_server_error("Artists list failed");
        }
    };

    Ok(rows
        .into_iter()
        .map(|row| Artist {
            id: row.id,
            name: row.name,
            slug: row.slug,
            description: row.description,
        })
        .collect())
}

/// Artist lookup used by the nested admin routes (real `artists` row).
/// Proves route params flow into server fns and `use_resource` SSR. The
/// explicit `HttpError` error type makes the wire response carry the real
/// status (verified: 200 on hit, 404 on miss).
#[get("/api/artist?artist_slug", db: axum::Extension<crate::db::Db>)]
pub async fn get_artist(artist_slug: String) -> core::result::Result<Artist, HttpError> {
    #[derive(sqlx::FromRow)]
    struct ArtistRow {
        id: i64,
        name: String,
        slug: String,
        description: Option<String>,
    }

    let row = match sqlx::query_as::<_, ArtistRow>(
        "SELECT id, name, slug, description FROM artists WHERE slug = $1",
    )
    .bind(&artist_slug)
    .fetch_optional(&db.0.0)
    .await
    {
        Ok(row) => row,
        Err(error) => {
            tracing::error!("artist lookup: database error: {error}");
            return HttpError::internal_server_error("Artist lookup failed");
        }
    };

    let Some(row) = row else {
        return HttpError::not_found(format!("No artist with slug '{artist_slug}'"));
    };

    Ok(Artist {
        id: row.id,
        name: row.name,
        slug: row.slug,
        description: row.description,
    })
}

// --- Streaming uploads and progress ---

/// Upload progress per original filename, shared between `upload_file` and
/// `file_progress` (spike stand-in for the app's `services::files` store).
#[cfg(feature = "server")]
#[derive(Clone, Copy)]
struct UploadProgress {
    total: u64,
    done: bool,
}

#[cfg(feature = "server")]
fn upload_tracker() -> &'static dashmap::DashMap<String, UploadProgress> {
    static TRACKER: std::sync::OnceLock<dashmap::DashMap<String, UploadProgress>> =
        std::sync::OnceLock::new();
    TRACKER.get_or_init(dashmap::DashMap::new)
}

/// admin/label_owner gate, matching the main app's `user_with_permissions`.
#[cfg(feature = "server")]
fn require_upload_permissions(current_user: Option<&User>) -> core::result::Result<(), HttpError> {
    let Some(user) = current_user else {
        return HttpError::unauthorized("Not logged in.");
    };
    if !(user.permissions.contains("admin") || user.permissions.contains("label_owner")) {
        return HttpError::forbidden("Uploading needs the admin or label_owner permission.");
    }
    Ok(())
}

/// Reduce arbitrary text to a filename-safe string. The main app's
/// `valid_file_name` is far more thorough (unicode, mime types, size limits);
/// for the spike a traversal-proof placeholder is enough.
#[cfg(feature = "server")]
fn sanitize_name(raw: &str) -> String {
    raw.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

/// Upload a file with two extra fields. Mirrors the main app's `UploadFile`
/// (multipart fields "type" then "slug" then the file, in that order): Dioxus
/// puts the two scalars in the query string and streams the file body.
#[post("/api/upload_file?file_type&slug", auth: crate::auth::Session)]
pub async fn upload_file(
    file_type: String,
    slug: String,
    mut upload: FileStream,
) -> core::result::Result<u64, HttpError> {
    require_upload_permissions(auth.current_user.as_ref())?;

    // The main app validates "type" against its upload config; the spike keeps
    // a tiny table to show the same validation is possible.
    let subdirectory = match file_type.as_str() {
        "artist_image" => "artists",
        "release_image" => "releases",
        unknown => {
            return HttpError::bad_request(format!("Unknown upload type '{unknown}'"));
        }
    };

    let Ok(upload_path) = std::env::var("UPLOAD_PATH") else {
        tracing::error!("UPLOAD_PATH is not set on the server");
        return HttpError::internal_server_error("UPLOAD_PATH is not set");
    };

    let original_name = upload.file_name().to_string();
    if original_name.is_empty() {
        return HttpError::bad_request("Upload has no filename.");
    }

    let safe_name = sanitize_name(&original_name);
    let safe_slug = sanitize_name(&slug);

    let tmp_dir = format!("{upload_path}/spike-tmp/{subdirectory}");
    if let Err(error) = tokio::fs::create_dir_all(&tmp_dir).await {
        tracing::error!("cannot create {tmp_dir}: {error}");
        return HttpError::internal_server_error("Cannot prepare the upload directory");
    }
    let tmp_dir = match tokio::fs::canonicalize(&tmp_dir).await {
        Ok(tmp_dir) => tmp_dir,
        Err(error) => {
            tracing::error!("cannot open {tmp_dir}: {error}");
            return HttpError::internal_server_error("Cannot open the upload directory");
        }
    };
    let tmp_path = tmp_dir.join(&safe_name);
    if !tmp_path.starts_with(&tmp_dir) {
        return HttpError::bad_request("Invalid file path");
    }

    let limit: u64 = 10 * 1024 * 1024; // spike cap; main app has per-type limits
    let tracker = upload_tracker();
    tracker.insert(
        original_name.clone(),
        UploadProgress {
            total: 0,
            done: false,
        },
    );

    use tokio::io::AsyncWriteExt;
    let mut write_result = Ok(());
    let mut uploaded: u64 = 0;

    let mut file = match tokio::fs::File::create(&tmp_path).await {
        Ok(file) => file,
        Err(error) => {
            tracker.remove(&original_name);
            tracing::error!("cannot create {}: {error}", tmp_path.display());
            return HttpError::internal_server_error("Cannot create the upload file");
        }
    };

    while let Some(chunk) = upload.next().await {
        write_result = match chunk {
            Ok(chunk) => {
                uploaded += chunk.len() as u64;
                if uploaded > limit {
                    HttpError::bad_request::<()>("Upload exceeds the 10 MiB spike cap")
                } else if let Err(error) = file.write_all(&chunk).await {
                    tracing::error!("write failed: {error}");
                    HttpError::internal_server_error::<()>("Upload write failed")
                } else if let Some(mut entry) = tracker.get_mut(&original_name) {
                    entry.total = uploaded;
                    Ok(())
                } else {
                    Ok(())
                }
            }
            Err(error) => {
                tracing::error!("upload stream error: {error}");
                HttpError::bad_request::<()>("Upload stream error")
            }
        };
        if write_result.is_err() {
            break;
        }
    }

    let _ = file.shutdown().await;

    match write_result {
        Ok(()) => {
            let final_dir = format!("{upload_path}/{subdirectory}");
            if let Err(error) = tokio::fs::create_dir_all(&final_dir).await {
                tracing::error!("cannot create {final_dir}: {error}");
            }
            let final_path = format!("{final_dir}/{safe_slug}-{safe_name}");
            if let Err(error) = tokio::fs::rename(&tmp_path, &final_path).await {
                tracing::error!("rename failed: {error}");
                return HttpError::internal_server_error("Upload finalization failed");
            }
            tracing::info!("stored upload as {final_path}");
        }
        Err(error) => {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            if let Some(mut entry) = tracker.get_mut(&original_name) {
                entry.done = true; // let any progress streamer finish cleanly
            }
            return Err(error);
        }
    }

    if let Some(mut entry) = tracker.get_mut(&original_name) {
        entry.total = uploaded;
        entry.done = true;
    }

    Ok(uploaded)
}

/// Progress stream for an in-flight upload: each item is the running byte
/// count as text (port of the main app's streaming `FileProgress` endpoint).
#[get("/api/file_progress?filename", auth: crate::auth::Session)]
pub async fn file_progress(filename: String) -> core::result::Result<TextStream, HttpError> {
    require_upload_permissions(auth.current_user.as_ref())?;

    let (tx, rx) = futures::channel::mpsc::unbounded();
    tokio::spawn(async move {
        // Poll the tracker (the main app used the same polling shape with a
        // dashmap). Ends when the upload finishes, never shows up, or the
        // client disconnects.
        let mut missing_rounds = 0;
        loop {
            match upload_tracker().get(&filename) {
                Some(entry) => {
                    let (total, done) = (entry.total, entry.done);
                    if tx.unbounded_send(total.to_string()).is_err() {
                        break; // client hung up
                    }
                    if done {
                        upload_tracker().remove(&filename);
                        break;
                    }
                }
                None => {
                    missing_rounds += 1;
                    if missing_rounds > 20 {
                        break; // upload never started
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    });

    Ok(Streaming::new(rx))
}
