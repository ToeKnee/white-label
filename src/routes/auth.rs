#[cfg(feature = "ssr")]
use bcrypt::{hash, verify, DEFAULT_COST};
use leptos::prelude::*;

use crate::models::auth::User;
#[cfg(feature = "ssr")]
use crate::models::auth::UserPasshash;
#[cfg(feature = "ssr")]
use crate::state::{auth, pool};

// #[cfg(feature = "ssr")]
// use axum_session_sqlx::SessionPgPool;
// #[cfg(feature = "ssr")]
// use sqlx::PgPool;
// #[cfg(feature = "ssr")]
// pub type AuthSession = axum_session_auth::AuthSession<User, i64, SessionPgPool, PgPool>;

#[server]
pub async fn get_user() -> Result<Option<User>, ServerFnError> {
    use crate::state::auth;

    let auth = auth()?;

    Ok(auth.current_user)
}

#[server(Login, "/api")]
pub async fn login(
    username: String,
    password: String,
    remember: Option<String>,
) -> Result<User, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;

    if username.is_empty() || password.is_empty() {
        return Err(ServerFnError::ServerError(
            "Username and password are required.".to_string(),
        ));
    }

    let (user, UserPasshash(expected_passhash)) =
        User::get_from_username_with_passhash(username, &pool)
            .await
            .ok_or_else(|| ServerFnError::new("User does not exist."))?;

    if verify(password, &expected_passhash)? {
        auth.login_user(user.id);
        auth.remember_user(remember.is_some());
        Ok(user)
    } else {
        Err(ServerFnError::ServerError(
            "Password does not match.".to_string(),
        ))
    }
}

#[server(Register, "/api")]
pub async fn register(
    username: String,
    password: String,
    password_confirmation: String,
    remember: Option<String>,
) -> Result<User, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;

    if username.is_empty() || password.is_empty() {
        return Err(ServerFnError::ServerError(
            "Username and password are required.".to_string(),
        ));
    }

    if password != password_confirmation {
        return Err(ServerFnError::ServerError(
            "Passwords did not match.".to_string(),
        ));
    }

    // Check if user already exists
    User::get_from_username(username.clone(), &pool)
        .await
        .map_or(Ok(()), |_| Err(ServerFnError::new("User already exists.")))?;

    let password_hashed = hash(password, DEFAULT_COST).unwrap();

    sqlx::query("INSERT INTO users (username, password) VALUES ($1,$2)")
        .bind(username.clone())
        .bind(password_hashed)
        .execute(&pool)
        .await?;

    let user = User::get_from_username(username, &pool)
        .await
        .ok_or_else(|| ServerFnError::new("User does not exist."))?;

    auth.login_user(user.id);
    auth.remember_user(remember.is_some());

    Ok(user)
}

#[server(Logout, "/api")]
pub async fn logout() -> Result<User, ServerFnError> {
    let auth = auth()?;

    auth.logout_user();

    Ok(User::default())
}
