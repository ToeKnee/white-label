//! Current-user plumbing for the Dioxus server.
//!
//! `tower-sessions` has no equivalent of the Leptos app's
//! `axum_session_auth::AuthLayer` (see MIGRATION.md risk 9), so this module
//! provides the two pieces the ported endpoints (Phase 2) will need:
//!
//! - `Db`: a newtype around `PgPool` so handlers and server fns can extract
//!   it (the orphan rule blocks implementing the extractor for `PgPool`
//!   itself; same approach as the Phase 0 spike).
//! - `CurrentUser`: reads the session record and loads the `User` by id,
//!   replacing `AuthSession<User, …>`.
//!
//! Example (Phase 2 handler shape):
//!
//! ```ignore
//! async fn profile(CurrentUser(user): CurrentUser, Db(pool): Db) -> Result<String, AppError> {
//!     let Some(user) = user else {
//!         return Err(AppError::unauthorized());
//!     };
//!     Ok(user.username)
//! }
//! ```

use std::convert::Infallible;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;

use crate::models::auth::User;

/// Session record key holding the logged-in user's id.
pub const USER_ID_SESSION_KEY: &str = "user_id";

/// The app's database pool, installed on requests via `Extension(Db(pool))`.
#[derive(Clone, Debug)]
pub struct Db(pub sqlx::PgPool);

/// The logged-in user for a request, if the session names one and that user
/// still exists.
///
/// The extractor itself never rejects: unauthenticated requests arrive with
/// `None`, letting each endpoint answer 401/403 with its own status (parity
/// with the Leptos routes' in-handler permission checks).
pub struct CurrentUser(pub Option<User>);

impl CurrentUser {
    /// Whether a real (non-guest) user was loaded.
    #[must_use]
    pub fn is_authenticated(&self) -> bool {
        self.0.as_ref().is_some_and(User::is_authenticated)
    }
}

impl<S> FromRequestParts<S> for CurrentUser
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Ok(session) = tower_sessions::Session::from_request_parts(parts, state).await else {
            return Ok(Self(None));
        };

        let user_id: Option<i64> = match session.get(USER_ID_SESSION_KEY).await {
            Ok(user_id) => user_id,
            Err(error) => {
                tracing::warn!("could not read session record: {error}");
                None
            }
        };

        let Some(Db(pool)) = parts.extensions.get::<Db>() else {
            tracing::warn!("Db extension missing — is the Extension(Db) layer installed?");
            return Ok(Self(None));
        };

        let user = match user_id {
            Some(user_id) => User::get(user_id, pool).await,
            None => None,
        };
        Ok(Self(user))
    }
}

/// Record a successful login in the session. The Dioxus replacement for
/// `AuthSession::login_user` — note that, unlike the `axum_session_auth`
/// quirk (spike finding 5), the caller already has the `User` in hand.
///
/// # Errors
///
/// Propagates session-store failures so a login request can answer 500
/// instead of silently dropping the session.
pub async fn login(
    session: &tower_sessions::Session,
    user_id: i64,
) -> Result<(), tower_sessions::session::Error> {
    session.insert(USER_ID_SESSION_KEY, &user_id).await
}

/// End the session for a logout (clears the record and rotates the cookie,
/// matching the Leptos app's logout behaviour).
///
/// # Errors
///
/// Propagates session-store failures.
pub async fn logout(
    session: &tower_sessions::Session,
) -> Result<(), tower_sessions::session::Error> {
    session.flush().await
}
