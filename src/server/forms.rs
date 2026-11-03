//! Server-side handlers for the app's plain HTML forms (MIGRATION.md
//! Phase 4 slices 3-5).
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
//!   optional CSRF-token pass are tracked as a follow-up in MIGRATION.md.
//! - Login failures answer the same fixed messages as the wire endpoint;
//!   bcrypt work happens only for an existing username.
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
use chrono::{DateTime, NaiveDateTime, Utc};
use tower_sessions::Session;

use crate::error::AppError;
use crate::forms::artist::{CreateArtistForm, UpdateArtistForm};
use crate::forms::page::{CreatePageForm, UpdatePageForm};
use crate::forms::release::{CreateReleaseForm, UpdateReleaseForm};
use crate::forms::user::{ChangePasswordForm, RegisterUserForm, UpdateUserForm};
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
        // Admin forms (slices 4-5). These post under the separate
        // `/admin/forms` namespace rather than the pages' own paths: the
        // EditPage route `/admin/page/:slug` would swallow POSTs sent to
        // `/admin/page/<anything>` (and likewise every dynamic admin GET
        // path), and registering methods on page paths would 405 their SSR
        // GETs (the slice-3 lesson).
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
}

/// Fields of the login form. Every field defaults so a missing one is a
/// validation failure to report, not an axum 400 HTML page.
#[derive(Debug, serde::Deserialize)]
struct LoginForm {
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
    /// Present (any value) when the "Remember me" checkbox is ticked.
    #[serde(default)]
    remember: Option<String>,
}

/// Fields of the register form (the `?bad=` codes match
/// `crate::ui::auth::register_message`).
#[derive(Debug, serde::Deserialize)]
struct RegisterForm {
    #[serde(default)]
    username: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    password_confirmation: String,
    #[serde(default)]
    remember: Option<String>,
}

/// Fields of the profile form. The optional text inputs always arrive (as
/// empty strings when blank), matching the Leptos form's behaviour.
#[derive(Debug, serde::Deserialize)]
struct ProfileForm {
    #[serde(default)]
    original_username: String,
    #[serde(default)]
    username: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    first_name: String,
    #[serde(default)]
    last_name: String,
}

/// Fields of the change-password form.
#[derive(Debug, serde::Deserialize)]
struct PasswordForm {
    #[serde(default)]
    password: String,
    #[serde(default)]
    new_password: String,
    #[serde(default)]
    new_password_confirmation: String,
}

/// Log in, then start the session — the web twin of the wire `POST
/// /api/login` (both run `crate::services::user::login_user`).
async fn login(Db(pool): Db, session: Session, Form(args): Form<LoginForm>) -> impl IntoResponse {
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

    // The home page's session read then shows the user menu, as the Leptos
    // page's `redirect("/")` after login did.
    Redirect::to("/")
}

/// Create the account, then log the new user in (the Leptos register page
/// redirected home the same way).
async fn register(
    Db(pool): Db,
    session: Session,
    Form(args): Form<RegisterForm>,
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
    Form(args): Form<ProfileForm>,
) -> impl IntoResponse {
    let Some(user) = current_user.as_ref() else {
        return Redirect::to("/login");
    };

    let form = UpdateUserForm {
        original_username: args.original_username,
        username: args.username,
        email: args.email,
        description: optional(args.description),
        first_name: optional(args.first_name),
        last_name: optional(args.last_name),
    };
    match update_user_service(&pool, Some(user), form).await {
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
    Form(args): Form<PasswordForm>,
) -> impl IntoResponse {
    let Some(user) = current_user.as_ref() else {
        return Redirect::to("/login");
    };

    let form = ChangePasswordForm {
        password: args.password,
        new_password: args.new_password,
        new_password_confirmation: args.new_password_confirmation,
    };
    match change_password_service(&pool, Some(user), form).await {
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

/// Treat an empty text input as "no value", the way the services' form
/// fields express optional text.
fn optional(text: String) -> Option<String> {
    (!text.is_empty()).then_some(text)
}

// The fixed codes `crate::ui::auth` maps onto alert text. Service messages
// are constants in `crate::services`, so comparing them is stable;
// anything unrecognised gets the generic `server`/`invalid` code.

fn login_code(error: &AppError) -> &'static str {
    match error {
        AppError::BadRequest(_) => "required",
        AppError::Unauthorized(message) if message == "User does not exist." => "not_found",
        AppError::Unauthorized(message) if message == "Password does not match." => "password",
        _ => "server",
    }
}

fn register_code(error: &AppError) -> &'static str {
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

fn profile_code(error: &AppError) -> &'static str {
    match error {
        AppError::Forbidden(_) => "forbidden",
        // `User::update` surfaces validation failures as Internal errors
        // whose message carries the uniqueness complaint.
        AppError::Internal(message) if message.contains("already taken") => "taken",
        _ => "server",
    }
}

fn change_password_code(error: &AppError) -> &'static str {
    match error {
        AppError::BadRequest(_) => "mismatch",
        AppError::Unauthorized(message) if message == "Password does not match." => "current",
        _ => "server",
    }
}

// --- Admin forms (MIGRATION.md Phase 4 slice 4) --------------------------
//
// The dashboard-side editors in `crate::ui::admin` post here; the fixed
// codes below are mapped to alert text there (`label_message`,
// `page_message`). Like the auth handlers, guests get steered to the login
// page and the services re-check the real permissions.

/// Fields of the label editor form. The hidden `id` parses leniently: a
/// tampered value becomes 0 and the service answers with its fixed retry
/// message.
#[derive(Debug, serde::Deserialize)]
struct LabelForm {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    isrc_base: String,
}

/// Fields of the page create form. Names are flat (the Leptos
/// `page_form[...]` prefix encoding is a server-fn struct convention this
/// plain-form surface does not need).
#[derive(Debug, serde::Deserialize)]
struct PageCreateForm {
    #[serde(default)]
    label_id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    body: String,
    /// Hidden full-date field (the stored value).
    #[serde(default)]
    published_at: String,
    /// Visible `datetime-local` value, preferred when present.
    #[serde(default)]
    local_published_at: String,
}

/// Fields of the page update form: the create fields plus the addressed
/// slug.
#[derive(Debug, serde::Deserialize)]
struct PageUpdateForm {
    #[serde(default)]
    slug: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    published_at: String,
    #[serde(default)]
    local_published_at: String,
}

/// The hidden slug of the delete/restore forms (pages and artists).
#[derive(Debug, serde::Deserialize)]
struct SlugForm {
    #[serde(default)]
    slug: String,
}

/// Save the record label's details (`label_owner` only; the service
/// enforces it). Success reloads the editor with `?updated=1` so the page
/// renders the saved values and the success alert.
async fn update_label(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    Form(args): Form<LabelForm>,
) -> impl IntoResponse {
    if current_user.is_none() {
        return Redirect::to("/login");
    }

    let id = args.id.parse::<i64>().unwrap_or_default();
    match services::record_label::update_record_label(
        &pool,
        current_user.as_ref(),
        id,
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
    Form(args): Form<PageCreateForm>,
) -> impl IntoResponse {
    if current_user.is_none() {
        return Redirect::to("/login");
    }

    let form = CreatePageForm {
        name: args.name,
        description: args.description,
        body: args.body,
        label_id: args.label_id.parse::<i64>().unwrap_or_default(),
        published_at: parse_form_date(&args.local_published_at, &args.published_at),
    };
    match services::page::create_page_service(&pool, current_user.as_ref(), form).await {
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
    Form(args): Form<PageUpdateForm>,
) -> impl IntoResponse {
    if current_user.is_none() {
        return Redirect::to("/login");
    }

    // Keep the requested slug for the failure redirect (the service takes
    // the form by value).
    let addressed_slug = args.slug.clone();
    let form = UpdatePageForm {
        slug: args.slug,
        name: args.name,
        description: args.description,
        body: args.body,
        published_at: parse_form_date(&args.local_published_at, &args.published_at),
    };
    match services::page::update_page_service(&pool, current_user.as_ref(), form).await {
        // `update` can re-slugify; redirect to the editor's *current* slug
        // like the Leptos action's result did.
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
/// shows the Restore button in place of Delete (the state swap the Leptos
/// dialog produced in place).
async fn delete_page(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    Form(args): Form<SlugForm>,
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
    Form(args): Form<SlugForm>,
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

/// Fields of the artist create form (flat names, like the page forms).
#[derive(Debug, serde::Deserialize)]
struct ArtistCreateForm {
    #[serde(default)]
    label_id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    website: String,
    /// Hidden full-date field (the stored value).
    #[serde(default)]
    published_at: String,
    /// Visible `datetime-local` value, preferred when present.
    #[serde(default)]
    local_published_at: String,
}

/// Fields of the artist update form: the create fields plus the addressed
/// slug.
#[derive(Debug, serde::Deserialize)]
struct ArtistUpdateForm {
    #[serde(default)]
    slug: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    website: String,
    #[serde(default)]
    published_at: String,
    #[serde(default)]
    local_published_at: String,
}

/// Create an artist. Success 303s to the new artist's profile editor under
/// the slug the server derived from the name (never echoing the submitted
/// name), with no `?updated=1`: the Leptos create action redirected
/// silently into the editor.
async fn create_artist(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    Form(args): Form<ArtistCreateForm>,
) -> impl IntoResponse {
    if current_user.is_none() {
        return Redirect::to("/login");
    }

    let form = CreateArtistForm {
        name: args.name,
        description: args.description,
        website: args.website,
        label_id: args.label_id.parse::<i64>().unwrap_or_default(),
        published_at: parse_form_date(&args.local_published_at, &args.published_at),
    };
    match services::artist::create_artist_service(&pool, current_user.as_ref(), form).await {
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
    Form(args): Form<ArtistUpdateForm>,
) -> impl IntoResponse {
    if current_user.is_none() {
        return Redirect::to("/login");
    }

    // Keep the requested slug for the failure redirect (the service takes
    // the form by value).
    let addressed_slug = args.slug.clone();
    let form = UpdateArtistForm {
        slug: args.slug,
        name: args.name,
        description: args.description,
        website: args.website,
        published_at: parse_form_date(&args.local_published_at, &args.published_at),
    };
    match services::artist::update_artist_service(&pool, current_user.as_ref(), form).await {
        // `Artist::update` re-slugifies from the name; redirect to the
        // editor's *current* slug, like the Leptos action's redirect did.
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
    Form(args): Form<SlugForm>,
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
    Form(args): Form<SlugForm>,
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

/// Fields of the release create form (flat names, minus `artist_ids`;
/// see `split_form_pairs` for why that key is handled separately).
#[derive(Debug, Default, serde::Deserialize)]
struct ReleaseCreateFields {
    #[serde(default)]
    label_id: String,
    /// The artist the page URL addresses (redirect targets).
    #[serde(default)]
    artist_slug: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    catalogue_number: String,
    #[serde(default)]
    primary_artist_id: String,
    /// Hidden full-date fields (the stored values).
    #[serde(default)]
    release_date: String,
    #[serde(default)]
    published_at: String,
    /// Visible `datetime-local` values, preferred when present.
    #[serde(default)]
    local_release_date: String,
    #[serde(default)]
    local_published_at: String,
}

/// Fields of the release update form: the create fields plus the addressed
/// slug.
#[derive(Debug, Default, serde::Deserialize)]
struct ReleaseUpdateFields {
    #[serde(default)]
    slug: String,
    #[serde(default)]
    label_id: String,
    #[serde(default)]
    artist_slug: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    catalogue_number: String,
    #[serde(default)]
    primary_artist_id: String,
    #[serde(default)]
    release_date: String,
    #[serde(default)]
    published_at: String,
    #[serde(default)]
    local_release_date: String,
    #[serde(default)]
    local_published_at: String,
}

/// Release delete/restore: the release slug plus the artist slug the
/// redirect targets are built under.
#[derive(Debug, serde::Deserialize)]
struct ReleaseSlugForm {
    #[serde(default)]
    slug: String,
    #[serde(default)]
    artist_slug: String,
}

/// The raw form pairs, with every value of `key` split out and the rest
/// rebuilt as a JSON object for a field-by-field `serde_json` pass.
///
/// The release (and later track) "All Artists" checkbox groups post
/// `artist_ids` once per checked artist — plain HTML, no JS. A derived
/// `Form<T>` struct cannot express that: one occurrence deserializes as a
/// bare string (wrong type for a `Vec` field) and repeated occurrences trip
/// serde's duplicate-field check, so the pairs are taken raw instead.
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
/// slug the server derived from the name, with no `?updated=1` (the Leptos
/// create action redirected silently), mirroring `create_artist`.
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
    let args: ReleaseCreateFields = serde_json::from_value(rest).unwrap_or_default();
    let form = CreateReleaseForm {
        artist_ids: artist_ids.join(","),
        name: args.name,
        description: args.description,
        primary_artist_id: args.primary_artist_id.parse::<i64>().unwrap_or_default(),
        catalogue_number: args.catalogue_number,
        release_date: parse_form_date(&args.local_release_date, &args.release_date),
        label_id: args.label_id.parse::<i64>().unwrap_or_default(),
        published_at: parse_form_date(&args.local_published_at, &args.published_at),
    };
    match services::release::create_release_service(&pool, current_user.as_ref(), form).await {
        Ok(result) => Redirect::to(&format!(
            "/admin/artist/{}/release/{}",
            args.artist_slug, result.release.slug
        )),
        Err(error) => {
            tracing::warn!("release create form rejected: {error}");
            Redirect::to(&format!(
                "/admin/artist/{}/releases/new?bad={}",
                args.artist_slug,
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
    let args: ReleaseUpdateFields = serde_json::from_value(rest).unwrap_or_default();
    // Keep the addressed slugs for the failure redirect (the service takes
    // the form by value).
    let addressed_slug = args.slug.clone();
    let addressed_artist = args.artist_slug.clone();
    let form = UpdateReleaseForm {
        artist_ids: artist_ids.join(","),
        slug: args.slug,
        name: args.name,
        description: args.description,
        primary_artist_id: args.primary_artist_id.parse::<i64>().unwrap_or_default(),
        catalogue_number: args.catalogue_number,
        release_date: parse_form_date(&args.local_release_date, &args.release_date),
        label_id: args.label_id.parse::<i64>().unwrap_or_default(),
        published_at: parse_form_date(&args.local_published_at, &args.published_at),
    };
    match services::release::update_release_service(&pool, current_user.as_ref(), form).await {
        // `Release::update` re-slugifies from the name; redirect to the
        // editor's *current* slug, like the Leptos action's redirect did.
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
    Form(args): Form<ReleaseSlugForm>,
) -> impl IntoResponse {
    if current_user.is_none() {
        return Redirect::to("/login");
    }

    let slug = args.slug;
    let artist_slug = args.artist_slug;
    match services::release::delete_release_service(&pool, current_user.as_ref(), slug.clone())
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
    Form(args): Form<ReleaseSlugForm>,
) -> impl IntoResponse {
    if current_user.is_none() {
        return Redirect::to("/login");
    }

    let slug = args.slug;
    let artist_slug = args.artist_slug;
    match services::release::restore_release_service(&pool, current_user.as_ref(), slug.clone())
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

/// Resolve a `DateField` submission: a submitted `datetime-local` value
/// wins (interpreted as UTC — the browser's zone is not applied; see
/// `ui::admin::pages`), else the hidden full-date field, else no date.
fn parse_form_date(local: &str, hidden: &str) -> Option<DateTime<Utc>> {
    if !local.is_empty() {
        // `datetime-local` sends minute precision; some browsers append
        // seconds (or fractions).
        for format in ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M"] {
            if let Ok(naive) = NaiveDateTime::parse_from_str(local, format) {
                return Some(naive.and_utc());
            }
        }
    }
    if !hidden.is_empty() {
        // The hidden field carries `DateTime<Utc>`'s Display form.
        if let Ok(when) = DateTime::parse_from_rfc3339(hidden) {
            return Some(when.to_utc());
        }
    }
    None
}

// The fixed codes `crate::ui::admin` maps onto alert text; service and
// model-validation messages are fixed strings, so comparing them is
// stable. Unrecognised failures get the generic `server` code.

const fn label_code(error: &AppError) -> &'static str {
    match error {
        AppError::Forbidden(_) => "forbidden",
        _ => "server",
    }
}

fn page_code(error: &AppError) -> &'static str {
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
/// `forbidden` code on submit (parity with the Leptos editors).
fn artist_code(error: &AppError) -> &'static str {
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
fn release_code(error: &AppError) -> &'static str {
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

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::{label_code, page_code, parse_form_date, release_code, split_form_pairs};
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
    fn form_date_prefers_the_local_input() {
        let parsed =
            parse_form_date("2026-03-01T09:30", "2020-01-01T00:00:00+00:00").expect("date");
        assert_eq!(parsed, Utc.with_ymd_and_hms(2026, 3, 1, 9, 30, 0).unwrap());
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
    fn form_date_accepts_seconds_in_the_local_input() {
        let parsed = parse_form_date("2026-03-01T09:30:15", "").expect("date");
        assert_eq!(parsed, Utc.with_ymd_and_hms(2026, 3, 1, 9, 30, 15).unwrap());
    }

    #[test]
    fn form_date_falls_back_to_the_hidden_field() {
        let parsed = parse_form_date("", "2026-02-15T12:00:00+00:00").expect("date");
        assert_eq!(parsed, Utc.with_ymd_and_hms(2026, 2, 15, 12, 0, 0).unwrap());
        // An unparseable local value also falls through.
        let parsed = parse_form_date("not-a-date", "2026-02-15T12:00:00+00:00").expect("date");
        assert_eq!(parsed, Utc.with_ymd_and_hms(2026, 2, 15, 12, 0, 0).unwrap());
    }

    #[test]
    fn form_date_empty_both_means_no_date() {
        assert!(parse_form_date("", "").is_none());
        assert!(parse_form_date("garbage", "garbage").is_none());
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
