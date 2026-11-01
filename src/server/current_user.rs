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
//!         return Err(AppError::Unauthorized("You must be logged in to view this page.".into()));
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::extract::FromRequestParts;
    use axum::http::Request;
    use axum::http::request::Parts;
    use sqlx::PgPool;
    use tower_sessions::Session;
    use tower_sessions_sqlx_store::PostgresStore;

    use super::{CurrentUser, Db, login, logout};
    use crate::models::test_helpers::create_test_user;

    /// Postgres-backed session store, migrated into the `#[sqlx::test]` db.
    async fn session_store(pool: &PgPool) -> Arc<PostgresStore> {
        let store = PostgresStore::new(pool.clone());
        store
            .migrate()
            .await
            .expect("session schema should migrate into the test db");
        Arc::new(store)
    }

    /// Bare request parts; the session and `Db` extensions are inserted by
    /// callers the way `SessionManagerLayer` and the router would.
    fn request_parts() -> Parts {
        Request::builder()
            .body(())
            .expect("infallible empty request")
            .into_parts()
            .0
    }

    async fn extract(parts: &mut Parts) -> CurrentUser {
        CurrentUser::from_request_parts(parts, &())
            .await
            .expect("the extractor never rejects")
    }

    #[sqlx::test]
    async fn login_persists_and_a_resumed_session_loads_the_user(pool: PgPool) {
        let created = create_test_user(&pool, 1).await.expect("test user");
        let store = session_store(&pool).await;

        let session = Session::new(None, store.clone(), None);
        login(&session, created.id)
            .await
            .expect("login records the user id");
        session
            .save()
            .await
            .expect("session should persist to Postgres");

        // A later request only carries the session id, so resume from it —
        // this exercises the Postgres store's load path, not just memory.
        let resumed = Session::new(session.id(), store, None);
        let mut parts = request_parts();
        parts.extensions.insert(resumed);
        parts.extensions.insert(Db(pool.clone()));

        let CurrentUser(loaded) = extract(&mut parts).await;
        let loaded = loaded.expect("the resumed session should load the user");
        assert_eq!(loaded.id, created.id);
        assert_eq!(loaded.username, format!("test-{}", created.id));
        assert!(CurrentUser(Some(loaded)).is_authenticated());
    }

    #[sqlx::test]
    async fn session_for_a_missing_user_yields_none(pool: PgPool) {
        let store = session_store(&pool).await;
        let session = Session::new(None, store, None);
        // The user is deleted (or the id never existed): the session still
        // names an id, but the DB lookup must come back empty.
        login(&session, 999_999)
            .await
            .expect("login records the user id");

        let mut parts = request_parts();
        parts.extensions.insert(session);
        parts.extensions.insert(Db(pool.clone()));

        let CurrentUser(loaded) = extract(&mut parts).await;
        assert!(loaded.is_none(), "unknown user id should yield no user");
    }

    #[sqlx::test]
    async fn logout_clears_the_user_from_the_session(pool: PgPool) {
        let created = create_test_user(&pool, 2).await.expect("test user");
        let store = session_store(&pool).await;

        let session = Session::new(None, store, None);
        login(&session, created.id)
            .await
            .expect("login records the user id");
        logout(&session).await.expect("logout flushes the session");

        let mut parts = request_parts();
        parts.extensions.insert(session);
        parts.extensions.insert(Db(pool.clone()));

        let CurrentUser(loaded) = extract(&mut parts).await;
        assert!(loaded.is_none(), "a flushed session should yield no user");
    }

    #[sqlx::test]
    async fn no_session_yields_none(pool: PgPool) {
        let mut parts = request_parts();
        parts.extensions.insert(Db(pool.clone()));

        let current_user = extract(&mut parts).await;
        assert!(!current_user.is_authenticated());
    }

    #[sqlx::test]
    async fn session_without_the_db_extension_yields_none(pool: PgPool) {
        let store = session_store(&pool).await;
        let session = Session::new(None, store, None);
        login(&session, 1).await.expect("login records the user id");

        // `Db` deliberately not inserted: the extractor must degrade to
        // "logged out" instead of panicking on a misconfigured router.
        let mut parts = request_parts();
        parts.extensions.insert(session);

        let CurrentUser(loaded) = extract(&mut parts).await;
        assert!(loaded.is_none(), "missing Db should yield no user");
    }
}
