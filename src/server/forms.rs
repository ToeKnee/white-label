//! Server-side handlers for the auth pages' plain HTML forms (MIGRATION.md
//! Phase 4 slice 3).
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
use tower_sessions::Session;

use crate::error::AppError;
use crate::forms::user::{ChangePasswordForm, RegisterUserForm, UpdateUserForm};
use crate::server::current_user::{
    CurrentUser, Db, login as record_login, logout as record_logout, remember_session,
};
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
