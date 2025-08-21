//! Global state for the application.
use axum::extract::FromRef;
use leptos::prelude::LeptosOptions;
use leptos::prelude::*;
use leptos_axum::{AxumRouteListing, extract};
use sqlx::PgPool;

use crate::app::UserContext;
use crate::models::auth::ssr::AuthSession;

/// The global state for the application.
///
/// This takes advantage of Axum's `SubStates` feature by deriving `FromRef`. This is the only way to have more than one
/// item in Axum's State. Leptos requires you to have leptosOptions in your State struct for the leptos route handlers
#[derive(FromRef, Debug, Clone)]
pub struct AppState {
    /// Options for the application - See <https://docs.rs/leptos_config/latest/leptos_config/struct.LeptosOptions.html> for detiails
    pub leptos_options: LeptosOptions,
    /// The database connection pool
    pub pool: PgPool,
    /// The routes for the application
    pub routes: Vec<AxumRouteListing>,
}

/// This gets the database pool from state
///
/// # Errors
///
/// Will return a `ServerError` error if the pool is missing.
pub fn pool() -> Result<PgPool, ServerFnError> {
    with_context::<AppState, _>(|state| state.pool.clone())
        .ok_or_else(|| ServerFnError::ServerError("Pool missing.".into()))
}

/// This gets the auth session from state
///
/// # Errors
///
/// Will return a `ServerError` error if the auth session is missing.
pub async fn auth() -> Result<AuthSession, ServerFnError> {
    let auth = extract().await?;
    Ok(auth)
}

/// This gets the user context from state
///
/// # Errors
///
/// Will return a `ServerError` error if the user state is missing.
pub fn user_context() -> Result<UserContext, ServerFnError> {
    use_context::<UserContext>()
        .ok_or_else(|| ServerFnError::ServerError("User context missing.".into()))
}
