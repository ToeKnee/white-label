#[cfg(feature = "ssr")]
use bcrypt::verify;
use leptos::prelude::*;
use server_fn::codec::Cbor;

use crate::forms::user::{ChangePasswordForm, RegisterUserForm, UpdateUserForm};
use crate::models::auth::User;
#[cfg(feature = "ssr")]
use crate::models::auth::UserPasshash;
#[cfg(feature = "ssr")]
use crate::services::user::{change_password_service, register_user_service, update_user_service};
#[cfg(feature = "ssr")]
use crate::state::{auth, pool};

#[server(GetUser, "/api", endpoint="user", output = Cbor)]
pub async fn get_user() -> Result<Option<User>, ServerFnError> {
    use crate::state::auth;

    let auth = auth()?;

    Ok(auth.current_user)
}

#[server(Login, "/api", endpoint="login", output = Cbor)]
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

#[server(Register, "/api", endpoint="register", output = Cbor)]
pub async fn register(
    form: RegisterUserForm,
    remember: Option<String>,
) -> Result<User, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;

    let user = register_user_service(&pool, form).await;
    match user {
        Ok(user) => {
            auth.login_user(user.id);
            auth.remember_user(remember.is_some());
            Ok(user)
        }
        Err(error) => Err(error),
    }
}

#[server(Logout, "/api", endpoint="logout", output = Cbor)]
pub async fn logout() -> Result<User, ServerFnError> {
    let auth = auth()?;

    auth.logout_user();

    Ok(User::default())
}

#[server(UpdateUser, "/api", endpoint="update_profile", output = Cbor)]
pub async fn update_user(user_form: UpdateUserForm) -> Result<User, ServerFnError> {
    let pool = pool()?;
    let mut auth = auth()?;
    let user = auth.current_user.as_ref();

    let response = update_user_service(&pool, user, user_form).await;
    auth.reload_user().await;
    response
}

#[server(ChangePassword, "/api", endpoint="change_password", output = Cbor)]
pub async fn change_password(password_form: ChangePasswordForm) -> Result<User, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();

    change_password_service(&pool, user, password_form).await
}
