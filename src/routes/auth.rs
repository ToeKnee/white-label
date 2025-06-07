//! Routes for user authentication and management.
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

/// Get the current user from the session.
///
/// # Returns:
/// * An `Option<User>` representing the current user if authenticated, or `None` if not.
///
/// # Errors:
/// Will return a `ServerFnError` if the authentication session is not available.
#[server(GetUser, "/api", endpoint="user", output = Cbor)]
pub async fn get_user() -> Result<Option<User>, ServerFnError> {
    use crate::state::auth;

    let auth = auth()?;

    Ok(auth.current_user)
}

/// Login a user with the provided username and password.
///
/// # Arguments:
/// * `username`: The username of the user.
/// * `password`: The password of the user.
/// * `remember`: An optional string indicating whether to remember the user (e.g., "true" or "false"). The session will be remembered for a longer period if this is set.
///
/// # Returns:
/// * A `Result<User, ServerFnError>` where `Ok(User)` contains the authenticated user if successful, or an error if the login fails.
///
/// # Errors:
/// Will return a `ServerFnError` if:
/// * The username or password is empty
/// * If the user does not exist
/// * If the password does not match the stored hash
#[server(Login, "/api", endpoint="login", output = Cbor)]
pub async fn login(
    /// The username of the user.
    username: String,
    /// The password of the user.
    password: String,
    /// An optional string indicating whether to remember the user (e.g., "true" or "false"). The session will be remembered for a longer period if this is set.
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

/// Register a new user with the provided form data.
///
/// # Arguments:
/// * `form`: The form data containing the details of the user to be registered.
/// * `remember`: An optional string indicating whether to remember the user (e.g., "true" or "false"). The session will be remembered for a longer period if this is set.
///
/// # Returns:
/// * A `Result<User, ServerFnError>` where `Ok(User)` contains the newly registered user if successful, or an error if the registration fails.
///
/// # Errors:
/// Will return a `ServerFnError` if:
/// * The username or password is empty
/// * If the user already exists
/// * If the registration fails for any other reason
#[server(Register, "/api", endpoint="register", output = Cbor)]
pub async fn register(
    /// The form data containing the details of the user to be registered.
    form: RegisterUserForm,
    /// An optional string indicating whether to remember the user (e.g., "true" or "false"). The session will be remembered for a longer period if this is set.
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

/// Logout the current user.
///
/// # Returns:
/// * A `Result<User, ServerFnError>` where `Ok(User)` contains a default user object indicating the user has been logged out.
///
/// # Errors:
/// Will return a `ServerFnError` if the authentication session is not available.
#[server(Logout, "/api", endpoint="logout", output = Cbor)]
pub async fn logout() -> Result<User, ServerFnError> {
    let auth = auth()?;

    auth.logout_user();

    Ok(User::default())
}

/// Update the current user's profile with the provided form data.
///
/// # Arguments:
/// * `user_form`: The form data containing the updated details of the user.
///
/// # Returns:
/// * A `Result<User, ServerFnError>` where `Ok(User)` contains the updated user if successful, or an error if the update fails.
///
/// # Errors:
/// Will return a `ServerFnError` if:
/// * The authentication session is not available
/// * If the user does not exist
/// * If the update fails for any other reason
#[server(UpdateUser, "/api", endpoint="update_profile", output = Cbor)]
pub async fn update_user(
    /// The form data containing the updated details of the user.
    user_form: UpdateUserForm,
) -> Result<User, ServerFnError> {
    let pool = pool()?;
    let mut auth = auth()?;
    let user = auth.current_user.as_ref();

    let response = update_user_service(&pool, user, user_form).await;
    auth.reload_user().await;
    response
}

/// Change the current user's password with the provided form data.
///
/// # Arguments:
/// * `password_form`: The form data containing the current password and the new password.
///
/// # Returns:
/// * A `Result<User, ServerFnError>` where `Ok(User)` contains the updated user if successful, or an error if the password change fails.
///
/// # Errors:
/// Will return a `ServerFnError` if:
/// * The authentication session is not available
/// * If the user does not exist
/// * If the current password does not match the stored hash
/// * If the new password does not meet the required criteria
#[server(ChangePassword, "/api", endpoint="change_password", output = Cbor)]
pub async fn change_password(
    /// The form data containing the current password and the new password.
    password_form: ChangePasswordForm,
) -> Result<User, ServerFnError> {
    let pool = pool()?;
    let auth = auth()?;
    let user = auth.current_user.as_ref();

    change_password_service(&pool, user, password_form).await
}
