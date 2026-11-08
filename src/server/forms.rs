//! Server-side handlers for the app's plain HTML forms.
//!
//! The `crate::ui::auth` pages render `<form method="post">` elements that
//! post to these handlers under the `/auth` namespace — deliberately *not*
//! the pages' own paths: registering `POST /login` would leave axum's
//! method router on that path and answer 405 to the SSR GET. Each handler
//! runs the same service call the wire `/api/*` port runs, then answers a
//! real **303 See Other**: to the home page on login/register success, or
//! back to the page carrying a fixed status code in the query string —
//! `?bad=<code>` for failures, `?saved=1`, `?updated=1`, `?done=1` for
//! successes. `ui::auth` owns the code→message tables; the handlers pick
//! codes from the [`AppError`] the service returned, the service messages
//! are fixed strings so the match is stable, and unknown errors fall back
//! to a generic `server` code (the detail stays in the tracing log).
//!
//! Safety notes for this network-facing surface:
//!
//! - The handlers never redirect to a caller-supplied URL — every target
//!   is a literal — and no submitted value is echoed back.
//! - The session cookie (tower-sessions defaults) is `HttpOnly` +
//!   SameSite=Lax, which blocks cross-site form posts from modern
//!   browsers. `Secure`/`SameSite=None` decisions for deployment and an
//!   optional CSRF-token pass remain open follow-ups for deployment.
//! - Login and register failures answer fixed codes (see [`login_code`]
//!   and [`register_code`]), the same ones the Dioxus auth server fns
//!   return; bcrypt work happens only for an existing username.
//!
//! Example flow (curl):
//!
//! ```text
//! POST /auth/login    username=alice&password=hunter2&remember=on
//! 303 -> /            (session cookie set)
//! ```

use axum::Router;
use axum::extract::Form;
use axum::response::{IntoResponse, Redirect};
use axum::routing::post;
use tower_sessions::Session;

use crate::error::AppError;
use crate::forms::PurgeFields;
use crate::forms::SlugFields;
use crate::forms::artist::{ArtistCreateFields, ArtistUpdateFields};
use crate::forms::auth::{
    ClientLoginForm, ClientPasswordFields, ClientProfileFields, ClientRegisterForm,
};
use crate::forms::links::LinksForm;
use crate::forms::page::{PageCreateFields, PageUpdateFields};
use crate::forms::record_label::LabelFields;
use crate::forms::release::{ReleaseCreateFields, ReleaseSlugFields, ReleaseUpdateFields};
use crate::forms::track::{TrackCreateFields, TrackSlugFields, TrackUpdateFields};
use crate::forms::user::RegisterUserForm;
use crate::server::current_user::{
    CurrentUser, Db, login as record_login, logout as record_logout, remember_session,
};
use crate::services;
use crate::services::user::{
    change_password_service, login_user, register_user_service, update_user_service,
};

/// The form-action routes, merged into `super::build_router`. They live
/// under `/auth` so the SSR pages keep answering GET on their own paths
/// (an axum method router on the page's path would 405 those GETs).
pub fn form_routes() -> Router {
    Router::new()
        .route("/auth/login", post(login))
        .route("/auth/register", post(register))
        .route("/auth/logout", post(logout))
        .route("/auth/profile", post(update_profile))
        .route("/auth/change-password", post(change_password))
        // Admin forms. These post under the separate
        // `/admin/forms` namespace rather than the pages' own paths: the
        // EditPage route `/admin/page/:slug` would swallow POSTs sent to
        // `/admin/page/<anything>` (and likewise every dynamic admin GET
        // path), and registering methods on page paths would 405 their SSR
        // GETs.
        .route("/admin/forms/label", post(update_label))
        .route("/admin/forms/page", post(create_page))
        .route("/admin/forms/page/update", post(update_page))
        .route("/admin/forms/page/delete", post(delete_page))
        .route("/admin/forms/page/restore", post(restore_page))
        .route("/admin/forms/artist", post(create_artist))
        .route("/admin/forms/artist/update", post(update_artist))
        .route("/admin/forms/artist/delete", post(delete_artist))
        .route("/admin/forms/artist/restore", post(restore_artist))
        .route("/admin/forms/release", post(create_release))
        .route("/admin/forms/release/update", post(update_release))
        .route("/admin/forms/release/delete", post(delete_release))
        .route("/admin/forms/release/restore", post(restore_release))
        .route("/admin/forms/track", post(create_track))
        .route("/admin/forms/track/update", post(update_track))
        .route("/admin/forms/track/delete", post(delete_track))
        .route("/admin/forms/track/restore", post(restore_track))
        .route("/admin/forms/links", post(submit_links))
        // Housekeeping: the two-step purge (preview, then `confirm=1`).
        .route("/admin/forms/purge", post(purge_deleted))
        // The upload widget's javascript-off form. Not under
        // `/admin/forms` because the avatar upload lives on `/profile`.
        .route(
            super::upload_form::PATH_UPLOAD_FORM,
            post(super::upload_form::upload_image),
        )
}

/// Log in, then start the session — the web twin of the wire `POST
/// /api/login` (both run `crate::services::user::login_user`).
async fn login(
    Db(pool): Db,
    session: Session,
    Form(args): Form<ClientLoginForm>,
) -> impl IntoResponse {
    let user = match login_user(&pool, args.username, args.password).await {
        Ok(user) => user,
        Err(error) => {
            tracing::warn!("login form rejected: {error}");
            return Redirect::to(&format!("/login?bad={}", login_code(&error)));
        }
    };

    if let Err(error) = record_login(&session, user.id).await {
        tracing::error!("login form: could not store the session: {error}");
        return Redirect::to("/login?bad=server");
    }
    remember_session(&session, args.remember.as_deref());

    // The home page's session read then shows the user menu.
    Redirect::to("/")
}

/// Create the account, then log the new user in and send them home.
async fn register(
    Db(pool): Db,
    session: Session,
    Form(args): Form<ClientRegisterForm>,
) -> impl IntoResponse {
    let form = RegisterUserForm {
        username: args.username,
        email: args.email,
        password: args.password,
        password_confirmation: args.password_confirmation,
    };
    let user = match register_user_service(&pool, form).await {
        Ok(user) => user,
        Err(error) => {
            tracing::warn!("register form rejected: {error}");
            return Redirect::to(&format!("/register?bad={}", register_code(&error)));
        }
    };

    if let Err(error) = record_login(&session, user.id).await {
        tracing::error!("register form: could not store the session: {error}");
        return Redirect::to("/register?bad=server");
    }
    remember_session(&session, args.remember.as_deref());

    Redirect::to("/")
}

/// End the session and show the "Logged out" state on the page.
async fn logout(session: Session) -> impl IntoResponse {
    if let Err(error) = record_logout(&session).await {
        // No failure channel on this page: log it and show the button
        // again so the visitor can retry.
        tracing::error!("logout form: could not end the session: {error}");
        return Redirect::to("/logout");
    }
    Redirect::to("/logout?done=1")
}

/// Save the signed-in user's profile. Guests are steered to the login
/// page; the service re-checks `original_username` against the session.
async fn update_profile(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    Form(args): Form<ClientProfileFields>,
) -> impl IntoResponse {
    let Some(user) = current_user.as_ref() else {
        return Redirect::to("/login");
    };

    match update_user_service(&pool, Some(user), args.into_service_form()).await {
        Ok(_) => Redirect::to("/profile?saved=1"),
        Err(error) => {
            tracing::warn!("profile form rejected: {error}");
            Redirect::to(&format!("/profile?bad={}", profile_code(&error)))
        }
    }
}

/// Change the signed-in user's password.
async fn change_password(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    Form(args): Form<ClientPasswordFields>,
) -> impl IntoResponse {
    let Some(user) = current_user.as_ref() else {
        return Redirect::to("/login");
    };

    match change_password_service(&pool, Some(user), args.into_service_form()).await {
        Ok(_) => Redirect::to("/profile/change-password?updated=1"),
        Err(error) => {
            tracing::warn!("change-password form rejected: {error}");
            Redirect::to(&format!(
                "/profile/change-password?bad={}",
                change_password_code(&error)
            ))
        }
    }
}

// The fixed codes `crate::ui::auth` maps onto alert text. Service messages
// are constants in `crate::services`, so comparing them is stable;
// anything unrecognised gets the generic `server`/`invalid` code. The
// reactive twins (`ui::auth`'s `/api/*` server fns) answer the same
// codes, so both paths keep failing identically.

pub fn login_code(error: &AppError) -> &'static str {
    match error {
        AppError::BadRequest(_) => "required",
        AppError::Unauthorized(message) if message == "User does not exist." => "not_found",
        AppError::Unauthorized(message) if message == "Password does not match." => "password",
        _ => "server",
    }
}

pub fn register_code(error: &AppError) -> &'static str {
    match error {
        AppError::BadRequest(message) if message == "Username and password are required." => {
            "required"
        }
        AppError::BadRequest(message) if message == "Passwords did not match." => "mismatch",
        AppError::BadRequest(message) if message == "Username or Email already taken." => "taken",
        // Any other validation complaint (bad email, length limits).
        AppError::BadRequest(_) => "invalid",
        _ => "server",
    }
}

/// The fixed code for a refused profile save. The reactive twin in
/// `crate::ui::auth` answers the same code, so both paths fail identically.
pub fn profile_code(error: &AppError) -> &'static str {
    match error {
        AppError::Forbidden(_) => "forbidden",
        // `User::update` surfaces validation failures as Internal errors
        // whose message carries the uniqueness complaint.
        AppError::Internal(message) if message.contains("already taken") => "taken",
        _ => "server",
    }
}

/// The fixed code for a refused password change, answered by both the
/// javascript-off handler and its reactive twin.
pub fn change_password_code(error: &AppError) -> &'static str {
    match error {
        AppError::BadRequest(_) => "mismatch",
        AppError::Unauthorized(message) if message == "Password does not match." => "current",
        _ => "server",
    }
}

// --- Admin forms --------------------------
//
// The dashboard-side editors in `crate::ui::admin` post here; the fixed
// codes below are mapped to alert text there (`label_message`,
// `page_message`). Like the auth handlers, guests get steered to the login
// page and the services re-check the real permissions.

// The admin editors' posted fields arrive as the shared structs in
// `crate::forms` (each turns itself into the form its service takes, so the
// reactive twins in `crate::ui` and these handlers send the services the same
// thing).

/// Save the record label's details (`label_owner` only; the service
/// enforces it). Success reloads the editor with `?updated=1` so the page
/// renders the saved values and the success alert.
async fn update_label(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    Form(args): Form<LabelFields>,
) -> impl IntoResponse {
    if current_user.is_none() {
        return Redirect::to("/login");
    }

    match services::record_label::update_record_label(
        &pool,
        current_user.as_ref(),
        args.parsed_id(),
        args.name,
        args.description,
        args.isrc_base,
    )
    .await
    {
        Ok(_) => Redirect::to("/admin/label?updated=1"),
        Err(error) => {
            tracing::warn!("label form rejected: {error}");
            Redirect::to(&format!("/admin/label?bad={}", label_code(&error)))
        }
    }
}

/// Create a marketing/legal page. Success 303s to the new page's editor
/// under the slug the server derived from the name (URL-safe by
/// construction, and never echoing the submitted name).
async fn create_page(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    Form(args): Form<PageCreateFields>,
) -> impl IntoResponse {
    if current_user.is_none() {
        return Redirect::to("/login");
    }

    match services::page::create_page_service(
        &pool,
        current_user.as_ref(),
        args.into_service_form(),
    )
    .await
    {
        Ok(result) => Redirect::to(&format!("/admin/page/{}", result.page.slug)),
        Err(error) => {
            tracing::warn!("page create form rejected: {error}");
            Redirect::to(&format!("/admin/page?bad={}", page_code(&error)))
        }
    }
}

/// Update a page (name, description, body and publish date).
async fn update_page(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    Form(args): Form<PageUpdateFields>,
) -> impl IntoResponse {
    if current_user.is_none() {
        return Redirect::to("/login");
    }

    // Keep the requested slug for the failure redirect (the service takes
    // the form by value).
    let addressed_slug = args.slug.clone();
    match services::page::update_page_service(
        &pool,
        current_user.as_ref(),
        args.into_service_form(),
    )
    .await
    {
        // `update` can re-slugify; redirect to the editor's *current* slug.
        Ok(result) => Redirect::to(&format!("/admin/page/{}?updated=1", result.page.slug)),
        Err(error) => {
            tracing::warn!("page update form rejected: {error}");
            Redirect::to(&format!(
                "/admin/page/{}?bad={}",
                addressed_slug,
                page_code(&error)
            ))
        }
    }
}

/// Soft-delete a page. The editor reloads with `?updated=1`; its refetch
/// shows the Restore button in place of Delete.
async fn delete_page(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    Form(args): Form<SlugFields>,
) -> impl IntoResponse {
    if current_user.is_none() {
        return Redirect::to("/login");
    }

    let slug = args.slug;
    match services::page::delete_page_service(&pool, current_user.as_ref(), slug.clone()).await {
        Ok(result) => Redirect::to(&format!("/admin/page/{}?updated=1", result.page.slug)),
        Err(error) => {
            tracing::warn!("page delete form rejected: {error}");
            Redirect::to(&format!("/admin/page/{slug}?bad={}", page_code(&error)))
        }
    }
}

/// Restore a soft-deleted page.
async fn restore_page(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    Form(args): Form<SlugFields>,
) -> impl IntoResponse {
    if current_user.is_none() {
        return Redirect::to("/login");
    }

    let slug = args.slug;
    match services::page::restore_page_service(&pool, current_user.as_ref(), slug.clone()).await {
        Ok(result) => Redirect::to(&format!("/admin/page/{}?updated=1", result.page.slug)),
        Err(error) => {
            tracing::warn!("page restore form rejected: {error}");
            Redirect::to(&format!("/admin/page/{slug}?bad={}", page_code(&error)))
        }
    }
}

/// Create an artist. Success 303s to the new artist's profile editor under
/// the slug the server derived from the name (never echoing the submitted
/// name), with no `?updated=1`.
async fn create_artist(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    Form(args): Form<ArtistCreateFields>,
) -> impl IntoResponse {
    if current_user.is_none() {
        return Redirect::to("/login");
    }

    match services::artist::create_artist_service(
        &pool,
        current_user.as_ref(),
        args.into_service_form(),
    )
    .await
    {
        Ok(result) => Redirect::to(&format!("/admin/artist/{}", result.artist.slug)),
        Err(error) => {
            tracing::warn!("artist create form rejected: {error}");
            Redirect::to(&format!("/admin/artist?bad={}", artist_code(&error)))
        }
    }
}

/// Update an artist's profile (name, description, website and publish
/// date).
async fn update_artist(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    Form(args): Form<ArtistUpdateFields>,
) -> impl IntoResponse {
    if current_user.is_none() {
        return Redirect::to("/login");
    }

    // Keep the requested slug for the failure redirect (the service takes
    // the form by value).
    let addressed_slug = args.slug.clone();
    match services::artist::update_artist_service(
        &pool,
        current_user.as_ref(),
        args.into_service_form(),
    )
    .await
    {
        // `Artist::update` re-slugifies from the name; redirect to the
        // editor's *current* slug.
        Ok(result) => Redirect::to(&format!("/admin/artist/{}?updated=1", result.artist.slug)),
        Err(error) => {
            tracing::warn!("artist update form rejected: {error}");
            Redirect::to(&format!(
                "/admin/artist/{}?bad={}",
                addressed_slug,
                artist_code(&error)
            ))
        }
    }
}

/// Soft-delete an artist. The editor reloads with `?updated=1`; its
/// refetch shows the Restore button in place of Delete.
async fn delete_artist(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    Form(args): Form<SlugFields>,
) -> impl IntoResponse {
    if current_user.is_none() {
        return Redirect::to("/login");
    }

    let slug = args.slug;
    match services::artist::delete_artist_service(&pool, current_user.as_ref(), slug.clone()).await
    {
        Ok(result) => Redirect::to(&format!("/admin/artist/{}?updated=1", result.artist.slug)),
        Err(error) => {
            tracing::warn!("artist delete form rejected: {error}");
            Redirect::to(&format!("/admin/artist/{slug}?bad={}", artist_code(&error)))
        }
    }
}

/// Restore a soft-deleted artist.
async fn restore_artist(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    Form(args): Form<SlugFields>,
) -> impl IntoResponse {
    if current_user.is_none() {
        return Redirect::to("/login");
    }

    let slug = args.slug;
    match services::artist::restore_artist_service(&pool, current_user.as_ref(), slug.clone()).await
    {
        Ok(result) => Redirect::to(&format!("/admin/artist/{}?updated=1", result.artist.slug)),
        Err(error) => {
            tracing::warn!("artist restore form rejected: {error}");
            Redirect::to(&format!("/admin/artist/{slug}?bad={}", artist_code(&error)))
        }
    }
}

/// The raw form pairs, with every value of `key` split out and the rest
/// rebuilt as a JSON object for a field-by-field `serde_json` pass.
///
/// The release (and later track) "All Artists" checkbox groups post
/// `artist_ids` once per checked artist — plain HTML, no JS. A derived
/// `Form<T>` struct cannot express that: one occurrence deserializes as a
/// bare string (wrong type for a `Vec` field) and repeated occurrences trip
/// serde's duplicate-field check, so the pairs are taken raw instead. The
/// shared posted-field structs keep `artist_ids` as the comma-joined string
/// their `into_service_form` passes on, so this only has to join the values it
/// split out.
fn split_form_pairs(pairs: Vec<(String, String)>, key: &str) -> (Vec<String>, serde_json::Value) {
    let mut values = Vec::new();
    let mut rest = serde_json::Map::new();
    for (pair_key, value) in pairs {
        if pair_key == key {
            values.push(value);
        } else {
            rest.insert(pair_key, serde_json::Value::String(value));
        }
    }
    (values, serde_json::Value::Object(rest))
}

/// Create a release. Success 303s to the new release's editor under the
/// slug the server derived from the name, with no `?updated=1`, mirroring
/// `create_artist`.
async fn create_release(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    Form(pairs): Form<Vec<(String, String)>>,
) -> impl IntoResponse {
    if current_user.is_none() {
        return Redirect::to("/login");
    }

    // A parse failure here means a hand-crafted POST body; falling back to
    // the default (empty) fields routes it through the service's own
    // validation (the `name_required` alert) rather than inventing a target.
    let (artist_ids, rest) = split_form_pairs(pairs, "artist_ids");
    let mut args: ReleaseCreateFields = serde_json::from_value(rest).unwrap_or_default();
    args.artist_ids = artist_ids.join(",");
    // Keep the addressed artist for the redirect targets (`into_service_form`
    // consumes the rest of the fields).
    let addressed_artist = args.artist_slug.clone();
    match services::release::create_release_service(
        &pool,
        current_user.as_ref(),
        args.into_service_form(),
        // The 303 handler renders no payload, so the toggle plays no part
        // here; the ladder default is fine.
        false,
    )
    .await
    {
        Ok(result) => Redirect::to(&format!(
            "/admin/artist/{}/release/{}",
            addressed_artist, result.release.slug
        )),
        Err(error) => {
            tracing::warn!("release create form rejected: {error}");
            Redirect::to(&format!(
                "/admin/artist/{}/releases/new?bad={}",
                addressed_artist,
                release_code(&error)
            ))
        }
    }
}

/// Update a release (name, description, artists, catalogue number and the
/// two dates).
async fn update_release(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    Form(pairs): Form<Vec<(String, String)>>,
) -> impl IntoResponse {
    if current_user.is_none() {
        return Redirect::to("/login");
    }

    let (artist_ids, rest) = split_form_pairs(pairs, "artist_ids");
    let mut args: ReleaseUpdateFields = serde_json::from_value(rest).unwrap_or_default();
    args.artist_ids = artist_ids.join(",");
    // Keep the addressed slugs for the failure redirect (the service takes
    // the form by value).
    let addressed_slug = args.slug.clone();
    let addressed_artist = args.artist_slug.clone();
    match services::release::update_release_service(
        &pool,
        current_user.as_ref(),
        args.into_service_form(),
        // Payload-free 303 handler (see above).
        false,
    )
    .await
    {
        // `Release::update` re-slugifies from the name; redirect to the
        // editor's *current* slug.
        Ok(result) => Redirect::to(&format!(
            "/admin/artist/{}/release/{}?updated=1",
            addressed_artist, result.release.slug
        )),
        Err(error) => {
            tracing::warn!("release update form rejected: {error}");
            Redirect::to(&format!(
                "/admin/artist/{}/release/{}?bad={}",
                addressed_artist,
                addressed_slug,
                release_code(&error)
            ))
        }
    }
}

/// Soft-delete a release. The editor reloads with `?updated=1`; its
/// refetch shows the Restore button in place of Delete.
async fn delete_release(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    Form(args): Form<ReleaseSlugFields>,
) -> impl IntoResponse {
    if current_user.is_none() {
        return Redirect::to("/login");
    }

    let slug = args.slug;
    let artist_slug = args.artist_slug;
    match services::release::delete_release_service(
        &pool,
        current_user.as_ref(),
        slug.clone(),
        false,
    )
    .await
    {
        Ok(result) => Redirect::to(&format!(
            "/admin/artist/{}/release/{}?updated=1",
            artist_slug, result.release.slug
        )),
        Err(error) => {
            tracing::warn!("release delete form rejected: {error}");
            Redirect::to(&format!(
                "/admin/artist/{}/release/{}?bad={}",
                artist_slug,
                slug,
                release_code(&error)
            ))
        }
    }
}

/// Restore a soft-deleted release.
async fn restore_release(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    Form(args): Form<ReleaseSlugFields>,
) -> impl IntoResponse {
    if current_user.is_none() {
        return Redirect::to("/login");
    }

    let slug = args.slug;
    let artist_slug = args.artist_slug;
    match services::release::restore_release_service(
        &pool,
        current_user.as_ref(),
        slug.clone(),
        false,
    )
    .await
    {
        Ok(result) => Redirect::to(&format!(
            "/admin/artist/{}/release/{}?updated=1",
            artist_slug, result.release.slug
        )),
        Err(error) => {
            tracing::warn!("release restore form rejected: {error}");
            Redirect::to(&format!(
                "/admin/artist/{}/release/{}?bad={}",
                artist_slug,
                slug,
                release_code(&error)
            ))
        }
    }
}

/// Create a track. Success 303s to the new track's editor under the slug
/// the server derived from the name, silently (no `?updated=1`), like the
/// release and artist create handlers.
async fn create_track(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    Form(pairs): Form<Vec<(String, String)>>,
) -> impl IntoResponse {
    if current_user.is_none() {
        return Redirect::to("/login");
    }

    let (artist_ids, rest) = split_form_pairs(pairs, "artist_ids");
    let mut args: TrackCreateFields = serde_json::from_value(rest).unwrap_or_default();
    args.artist_ids = artist_ids.join(",");
    // Keep the addressed slugs for the redirect targets (`into_service_form`
    // consumes the rest of the fields).
    let addressed_artist = args.artist_slug.clone();
    let addressed_release = args.release_slug.clone();
    match services::track::create_track_service(
        &pool,
        current_user.as_ref(),
        args.into_service_form(),
    )
    .await
    {
        Ok(result) => Redirect::to(&format!(
            "/admin/artist/{}/release/{}/track/{}",
            addressed_artist, addressed_release, result.track.slug
        )),
        Err(error) => {
            tracing::warn!("track create form rejected: {error}");
            Redirect::to(&format!(
                "/admin/artist/{}/release/{}/tracks/new?bad={}",
                addressed_artist,
                addressed_release,
                track_code(&error)
            ))
        }
    }
}

/// Update a track (name, description, lyrics, artists, ISRC, BPM, track
/// number and the publish date).
async fn update_track(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    Form(pairs): Form<Vec<(String, String)>>,
) -> impl IntoResponse {
    if current_user.is_none() {
        return Redirect::to("/login");
    }

    let (artist_ids, rest) = split_form_pairs(pairs, "artist_ids");
    let mut args: TrackUpdateFields = serde_json::from_value(rest).unwrap_or_default();
    args.artist_ids = artist_ids.join(",");
    // Keep the addressed slugs for the failure redirect (the service takes
    // the form by value).
    let addressed_slug = args.slug.clone();
    let addressed_artist = args.artist_slug.clone();
    let addressed_release = args.release_slug.clone();
    match services::track::update_track_service(
        &pool,
        current_user.as_ref(),
        args.into_service_form(),
    )
    .await
    {
        // `Track::update` re-slugifies from the name; redirect to the
        // editor's *current* slug.
        Ok(result) => Redirect::to(&format!(
            "/admin/artist/{}/release/{}/track/{}?updated=1",
            addressed_artist, addressed_release, result.track.slug
        )),
        Err(error) => {
            tracing::warn!("track update form rejected: {error}");
            Redirect::to(&format!(
                "/admin/artist/{}/release/{}/track/{}?bad={}",
                addressed_artist,
                addressed_release,
                addressed_slug,
                track_code(&error)
            ))
        }
    }
}

/// Soft-delete a track. The editor reloads with `?updated=1`; its
/// refetch shows the Restore button in place of Delete.
async fn delete_track(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    Form(args): Form<TrackSlugFields>,
) -> impl IntoResponse {
    if current_user.is_none() {
        return Redirect::to("/login");
    }

    let slug = args.slug;
    let artist_slug = args.artist_slug;
    let release_slug = args.release_slug;
    match services::track::delete_track_service(&pool, current_user.as_ref(), slug.clone()).await {
        Ok(result) => Redirect::to(&format!(
            "/admin/artist/{}/release/{}/track/{}?updated=1",
            artist_slug, release_slug, result.track.slug
        )),
        Err(error) => {
            tracing::warn!("track delete form rejected: {error}");
            Redirect::to(&format!(
                "/admin/artist/{}/release/{}/track/{}?bad={}",
                artist_slug,
                release_slug,
                slug,
                track_code(&error)
            ))
        }
    }
}

/// Restore a soft-deleted track.
async fn restore_track(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    Form(args): Form<TrackSlugFields>,
) -> impl IntoResponse {
    if current_user.is_none() {
        return Redirect::to("/login");
    }

    let slug = args.slug;
    let artist_slug = args.artist_slug;
    let release_slug = args.release_slug;
    match services::track::restore_track_service(&pool, current_user.as_ref(), slug.clone()).await {
        Ok(result) => Redirect::to(&format!(
            "/admin/artist/{}/release/{}/track/{}?updated=1",
            artist_slug, release_slug, result.track.slug
        )),
        Err(error) => {
            tracing::warn!("track restore form rejected: {error}");
            Redirect::to(&format!(
                "/admin/artist/{}/release/{}/track/{}?bad={}",
                artist_slug,
                release_slug,
                slug,
                track_code(&error)
            ))
        }
    }
}

/// Fields of the links form: the hidden artist slug plus one input per
/// platform. Every platform always renders an input, so
/// an empty value means "clear this link"; absent fields default the same
/// way. There are no repeated keys here, unlike the artist checkboxes.
#[derive(Debug, Default, serde::Deserialize)]
struct LinksFields {
    #[serde(default)]
    artist_slug: String,
    #[serde(default)]
    amazon_music: String,
    #[serde(default)]
    apple_music: String,
    #[serde(default)]
    bandcamp: String,
    #[serde(default)]
    beatport: String,
    #[serde(default)]
    deezer: String,
    #[serde(default)]
    sound_cloud: String,
    #[serde(default)]
    spotify: String,
    #[serde(default)]
    tidal: String,
    #[serde(default)]
    you_tube_music: String,
    #[serde(default)]
    blue_sky: String,
    #[serde(default)]
    facebook: String,
    #[serde(default)]
    instagram: String,
    #[serde(default)]
    linked_in: String,
    #[serde(default)]
    mastodon: String,
    #[serde(default)]
    pinterest: String,
    #[serde(default)]
    snapchat: String,
    #[serde(default)]
    threads: String,
    #[serde(default)]
    tik_tok: String,
    #[serde(default)]
    twitter: String,
    #[serde(default)]
    you_tube: String,
}

/// Save an artist's links (the no-JS fallback for the links page; the
/// reactive path posts to the JSON twin `ui::data::update_links`). The
/// service applies the create/update/delete diff from the submitted URLs
/// itself, so a success 303 always carries `?updated=1` (unlike the other
/// create handlers, there is no "new row" slug to land on).
async fn submit_links(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    Form(pairs): Form<Vec<(String, String)>>,
) -> impl IntoResponse {
    if current_user.is_none() {
        return Redirect::to("/login");
    }

    // No checkbox group here; the split helper's object builder is reused
    // with a key the links form never sends.
    let (_, rest) = split_form_pairs(pairs, "artist_ids");
    let args: LinksFields = serde_json::from_value(rest).unwrap_or_default();
    // Keep the addressed slug for the redirect targets (the form is moved
    // into the service call).
    let addressed_artist = args.artist_slug.clone();
    let form = LinksForm {
        artist_slug: args.artist_slug,
        amazon_music: args.amazon_music,
        apple_music: args.apple_music,
        bandcamp: args.bandcamp,
        beatport: args.beatport,
        deezer: args.deezer,
        sound_cloud: args.sound_cloud,
        spotify: args.spotify,
        tidal: args.tidal,
        you_tube_music: args.you_tube_music,
        blue_sky: args.blue_sky,
        facebook: args.facebook,
        instagram: args.instagram,
        linked_in: args.linked_in,
        mastodon: args.mastodon,
        pinterest: args.pinterest,
        snapchat: args.snapchat,
        threads: args.threads,
        tik_tok: args.tik_tok,
        twitter: args.twitter,
        you_tube: args.you_tube,
    };
    match services::links::update_links_service(&pool, current_user.as_ref(), form).await {
        Ok(_) => Redirect::to(&format!("/admin/artist/{addressed_artist}/links?updated=1")),
        Err(error) => {
            tracing::warn!("links form rejected: {error}");
            Redirect::to(&format!(
                "/admin/artist/{addressed_artist}/links?bad={}",
                links_code(&error)
            ))
        }
    }
}

/// The links service only fails on permissions, a missing artist, or the
/// database/rows, so the code set is short.
const fn links_code(error: &AppError) -> &'static str {
    match error {
        AppError::Unauthorized(_) | AppError::Forbidden(_) => "forbidden",
        AppError::NotFound(_) => "no_artist",
        _ => "server",
    }
}

/// Preview (`confirm` empty) or run (`confirm=1`) the housekeeping purge,
/// answering through the dashboard's query codes: `?ready=<count>` after a
/// preview (a read-only listing; the card reloads it through
/// `crate::ui::data::get_purge_preview` and shows the names),
/// `?purged=<count>&skipped=<count>` after a real purge, `?bad=<code>` on
/// refusal. The dashboard card (`crate::ui::admin::dashboard::PurgeCard`)
/// maps the codes to text; only re-parsed counts travel, never submitted
/// strings.
async fn purge_deleted(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    Form(args): Form<PurgeFields>,
) -> impl IntoResponse {
    if current_user.is_none() {
        return Redirect::to("/login");
    }

    let confirm = args.confirm == "1";
    match services::purge::purge_deleted_service(&pool, current_user.as_ref(), confirm).await {
        Ok(result) if confirm => Redirect::to(&format!(
            "/admin?purged={}&skipped={}",
            result.total(),
            result.skipped_artists.len()
        )),
        Ok(result) => Redirect::to(&format!("/admin?ready={}", result.total())),
        Err(error) => {
            tracing::warn!("purge form rejected: {error}");
            Redirect::to(&format!("/admin?bad={}", purge_code(&error)))
        }
    }
}

/// A purge has no user-input failure modes: the session either lacks the
/// `admin` permission or the queries could not run. The reactive twin
/// in `crate::ui::admin::dashboard` answers with the same codes.
pub const fn purge_code(error: &AppError) -> &'static str {
    match error {
        AppError::Unauthorized(_) | AppError::Forbidden(_) => "forbidden",
        _ => "server",
    }
}

// The fixed codes `crate::ui::admin` maps onto alert text; service and
// model-validation messages are fixed strings, so comparing them is
// stable. Unrecognised failures get the generic `server` code. The tables are
// `pub(crate)` because the reactive twins in `crate::ui` answer refusals as
// data (`crate::api_types::edit::EditOutcome`) instead of putting the code in a
// 303's query string, and both paths must report the same code.

pub const fn label_code(error: &AppError) -> &'static str {
    match error {
        AppError::Forbidden(_) => "forbidden",
        _ => "server",
    }
}

pub fn page_code(error: &AppError) -> &'static str {
    match error {
        AppError::Forbidden(_) => "forbidden",
        // Model validation failures surface wrapped as `Internal` with
        // their fixed messages.
        AppError::Internal(message) if message.contains("Name is required.") => "name_required",
        AppError::Internal(message) if message.contains("Name must be less than") => "name_length",
        AppError::Internal(message) if message.contains("Slug must be unique") => "slug_taken",
        AppError::Internal(message) if message.contains("Description must be less than") => {
            "desc_length"
        }
        _ => "server",
    }
}

/// The artist services require both `admin` and `label_owner`, so a
/// `label_owner`-only session reaches the form and then gets this
/// `forbidden` code on submit.
pub fn artist_code(error: &AppError) -> &'static str {
    match error {
        AppError::Forbidden(_) => "forbidden",
        AppError::Internal(message) if message.contains("Name is required.") => "name_required",
        AppError::Internal(message) if message.contains("Name must be less than") => "name_length",
        AppError::Internal(message) if message.contains("Slug must be unique") => "slug_taken",
        AppError::Internal(message) if message.contains("Record Label with id") => "no_label",
        _ => "server",
    }
}

/// The release services also require both `admin` and `label_owner`; the
/// extra codes come from `Release::validate` (fixed model messages).
pub fn release_code(error: &AppError) -> &'static str {
    match error {
        AppError::Forbidden(_) => "forbidden",
        AppError::Internal(message) if message.contains("Name is required.") => "name_required",
        AppError::Internal(message) if message.contains("Name must be less than") => "name_length",
        AppError::Internal(message) if message.contains("Slug must be unique") => "slug_taken",
        AppError::Internal(message) if message.contains("Record Label with id") => "no_label",
        AppError::Internal(message) if message.contains("Artist with id") => "no_artist",
        AppError::Internal(message) if message.contains("Catalogue number must be less than") => {
            "catalogue_length"
        }
        AppError::Internal(message) if message.contains("Catalogue number must be unique") => {
            "catalogue_taken"
        }
        _ => "server",
    }
}

/// The track services also require both `admin` and `label_owner`; the
/// extra codes come from `Track::validate` (fixed model messages; the
/// track-number collision message embeds ids, so it is matched on its
/// stable middle).
pub fn track_code(error: &AppError) -> &'static str {
    match error {
        AppError::Forbidden(_) => "forbidden",
        AppError::Internal(message) if message.contains("Name is required.") => "name_required",
        AppError::Internal(message) if message.contains("Name must be less than") => "name_length",
        AppError::Internal(message) if message.contains("Slug must be unique") => "slug_taken",
        AppError::Internal(message) if message.contains("Artist with id") => "no_artist",
        AppError::Internal(message) if message.contains("Could not find release with id") => {
            "no_release"
        }
        AppError::Internal(message) if message.contains("ISRC code must be 12") => "isrc_length",
        AppError::Internal(message) if message.contains("ISRC code must be unique") => "isrc_taken",
        AppError::Internal(message) if message.contains("must be unique for release") => {
            "track_number_taken"
        }
        _ => "server",
    }
}

#[cfg(test)]
mod tests {
    use super::{label_code, links_code, page_code, release_code, split_form_pairs, track_code};
    use crate::error::AppError;

    #[test]
    fn split_form_pairs_collects_repeated_keys() {
        let pairs = vec![
            ("name".to_string(), "A B".to_string()),
            ("artist_ids".to_string(), "1".to_string()),
            ("artist_ids".to_string(), "3".to_string()),
        ];
        let (artist_ids, rest) = split_form_pairs(pairs, "artist_ids");
        assert_eq!(artist_ids, vec!["1".to_string(), "3".to_string()]);
        assert_eq!(rest["name"], serde_json::Value::String("A B".into()));
        assert!(rest.get("artist_ids").is_none());
    }

    #[test]
    fn split_form_pairs_accepts_a_single_or_absent_key() {
        let (artist_ids, rest) = split_form_pairs(
            vec![
                ("artist_ids".into(), "2".into()),
                ("slug".into(), "s".into()),
            ],
            "artist_ids",
        );
        assert_eq!(artist_ids, vec!["2".to_string()]);
        assert_eq!(rest["slug"], serde_json::Value::String("s".into()));

        let (none, rest) = split_form_pairs(vec![("name".into(), "n".into())], "artist_ids");
        assert_eq!(none, Vec::<String>::new());
        assert_eq!(rest["name"], serde_json::Value::String("n".into()));
    }

    #[test]
    fn links_codes_map_the_known_failures() {
        assert_eq!(links_code(&AppError::Forbidden("no".into())), "forbidden");
        assert_eq!(
            links_code(&AppError::Unauthorized("User not supplied.".into())),
            "forbidden"
        );
        assert_eq!(
            links_code(&AppError::NotFound("Artist not found".into())),
            "no_artist"
        );
        // Row-handling failures arrive wrapped as internal errors.
        assert_eq!(
            links_code(&AppError::Internal(
                "Error handling music services: boom".into()
            )),
            "server"
        );
    }

    #[test]
    fn track_codes_map_the_known_failures() {
        assert_eq!(track_code(&AppError::Forbidden("no".into())), "forbidden");
        assert_eq!(
            track_code(&AppError::Internal("Name is required.".into())),
            "name_required"
        );
        assert_eq!(
            track_code(&AppError::Internal(
                "Could not find release with id 3.".into()
            )),
            "no_release"
        );
        assert_eq!(
            track_code(&AppError::Internal(
                "ISRC code must be 12 characters.".into()
            )),
            "isrc_length"
        );
        assert_eq!(
            track_code(&AppError::Internal("ISRC code must be unique.".into())),
            "isrc_taken"
        );
        // The collision message embeds the track and release ids.
        assert_eq!(
            track_code(&AppError::Internal(
                "Track number 3 must be unique for release with id 7.".into()
            )),
            "track_number_taken"
        );
        assert_eq!(
            track_code(&AppError::Internal(
                "Artist with id 9 does not exist.".into()
            )),
            "no_artist"
        );
        // Unknown and not-found failures get the generic code.
        assert_eq!(track_code(&AppError::NotFound("gone".into())), "server");
    }

    #[test]
    fn release_codes_map_the_known_failures() {
        assert_eq!(release_code(&AppError::Forbidden("no".into())), "forbidden");
        assert_eq!(
            release_code(&AppError::Internal("Name is required.".into())),
            "name_required"
        );
        assert_eq!(
            release_code(&AppError::Internal(
                "Artist with id 3 does not exist.".into()
            )),
            "no_artist"
        );
        assert_eq!(
            release_code(&AppError::Internal(
                "Catalogue number must be unique.".into()
            )),
            "catalogue_taken"
        );
        assert_eq!(
            release_code(&AppError::Internal(
                "Catalogue number must be less than 255 characters.".into()
            )),
            "catalogue_length"
        );
        // Unknown and not-found failures get the generic code.
        assert_eq!(release_code(&AppError::NotFound("gone".into())), "server");
    }

    #[test]
    fn label_codes_map_service_errors() {
        assert_eq!(label_code(&AppError::Forbidden("nope".into())), "forbidden");
        assert_eq!(label_code(&AppError::Internal("db".into())), "server");
        assert_eq!(label_code(&AppError::NotFound("gone".into())), "server");
    }

    #[test]
    fn page_codes_map_validation_and_permission_errors() {
        assert_eq!(page_code(&AppError::Forbidden("nope".into())), "forbidden");
        assert_eq!(
            page_code(&AppError::Internal("Name is required.".into())),
            "name_required"
        );
        assert_eq!(
            page_code(&AppError::Internal(
                "Name must be less than 255 characters.".into()
            )),
            "name_length"
        );
        assert_eq!(
            page_code(&AppError::Internal("Slug must be unique.".into())),
            "slug_taken"
        );
        assert_eq!(
            page_code(&AppError::Internal(
                "Description must be less than 255 characters.".into()
            )),
            "desc_length"
        );
        assert_eq!(page_code(&AppError::Internal("boom".into())), "server");
        assert_eq!(page_code(&AppError::Unauthorized("gone".into())), "server");
    }
}
