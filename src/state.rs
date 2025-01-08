//! Global state for the application.
use crate::models::auth::ssr::AuthSession;
use axum::extract::FromRef;
use leptos::prelude::LeptosOptions;
use leptos::prelude::*;
use leptos_axum::AxumRouteListing;
use sqlx::PgPool;

/// This takes advantage of Axum's `SubStates` feature by deriving `FromRef`. This is the only way to have more than one
/// item in Axum's State. Leptos requires you to have leptosOptions in your State struct for the leptos route handlers
#[derive(FromRef, Debug, Clone)]
pub struct AppState {
    pub leptos_options: LeptosOptions,
    pub pool: PgPool,
    pub routes: Vec<AxumRouteListing>,
}

pub fn pool() -> Result<PgPool, ServerFnError> {
    use_context::<PgPool>().ok_or_else(|| ServerFnError::ServerError("Pool missing.".into()))
}

pub fn auth() -> Result<AuthSession, ServerFnError> {
    use_context::<AuthSession>()
        .ok_or_else(|| ServerFnError::ServerError("Auth session missing.".into()))
}
