//! Global state for the application.
use axum::extract::FromRef;
use leptos::prelude::LeptosOptions;
use leptos::prelude::*;
use leptos_axum::AxumRouteListing;
use sqlx::PgPool;

use crate::app::UserContext;
use crate::models::auth::ssr::AuthSession;

/// The global state for the application.
///
/// This takes advantage of Axum's `SubStates` feature by deriving `FromRef`. This is the only way to have more than one
/// item in Axum's State. Leptos requires you to have leptosOptions in your State struct for the leptos route handlers
#[derive(FromRef, Debug, Clone)]
pub struct AppState {
    pub leptos_options: LeptosOptions,
    pub pool: PgPool,
    pub routes: Vec<AxumRouteListing>,
}

/// This gets the database pool from state
///
/// # Errors
///
/// Will return a `ServerError` error if the pool is missing.
pub fn pool() -> Result<PgPool, ServerFnError> {
    use_context::<PgPool>().ok_or_else(|| ServerFnError::ServerError("Pool missing.".into()))
}

/// This gets the auth session from state
///
/// # Errors
///
/// Will return a `ServerError` error if the auth session is missing.
pub fn auth() -> Result<AuthSession, ServerFnError> {
    use_context::<AuthSession>().ok_or_else(|| ServerFnError::ServerError("Auth session missing.".into()))
}

/// This gets the user context from state
///
/// # Errors
///
/// Will return a `ServerError` error if the user state is missing.
pub fn user_context() -> Result<UserContext, ServerFnError> {
    use_context::<UserContext>().ok_or_else(|| ServerFnError::ServerError("User context missing.".into()))
}
