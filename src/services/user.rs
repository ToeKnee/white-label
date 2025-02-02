#[cfg(feature = "ssr")]
use bcrypt::{hash, DEFAULT_COST};
use leptos::prelude::ServerFnError;
use sqlx::PgPool;

use crate::forms::user::{RegisterUserForm, UpdateUserForm};
use crate::models::auth::User;
#[cfg(feature = "ssr")]
use crate::models::traits::Validate;

/// Register User
///
/// This will register a user with the given username, email and matching passwords
///
/// # Arguments
/// pool: `PgPool` - The database connection pool
/// form: `RegisterUserForm` - The form to register the user
///
/// # Returns
/// Result<`User`, `ServerFnError`> - The registered user
///
/// # Errors
/// If the username is empty, return an error
/// If the password is empty, return an error
/// If the passwords do not match, return an error
/// If the user cannot be validated, return an error
/// If the user cannot be inserted, return an error
/// If the user cannot be found, return an error
///
/// # Panics
/// If the password cannot be hashed, panic
#[cfg(feature = "ssr")]
pub async fn register_user_service(
    pool: PgPool,
    form: RegisterUserForm,
) -> Result<User, ServerFnError> {
    if form.username.clone().is_empty() || form.password.is_empty() {
        return Err(ServerFnError::ServerError(
            "Username and password are required.".to_string(),
        ));
    }

    if form.password != form.password_confirmation {
        return Err(ServerFnError::ServerError(
            "Passwords did not match.".to_string(),
        ));
    }

    let user = User {
        username: form.username,
        email: form.email,
        ..Default::default()
    };
    let result = user.validate(&pool).await;
    if let Err(error) = result {
        return Err(ServerFnError::ServerError(error.to_string()));
    }

    let password_hashed = match hash(form.password, DEFAULT_COST) {
        Ok(hash) => hash,
        Err(e) => {
            leptos::logging::error!("{:?}", e);
            return Err(ServerFnError::ServerError(
                "Error hassing password.".to_string(),
            ));
        }
    };

    sqlx::query("INSERT INTO users (username, email, password) VALUES ($1, $2, $3)")
        .bind(user.username.clone())
        .bind(user.email)
        .bind(password_hashed)
        .execute(&pool)
        .await?;

    User::get_from_username(user.username, &pool)
        .await
        .ok_or_else(|| ServerFnError::new("User does not exist."))
}

/// Update an user
///
/// # Arguments
/// pool: `PgPool` - The database connection pool
/// user: Option<&User> - The user updating the user
/// `user_form`: `UpdateUserForm` - The form to update the user
///
/// # Returns
/// Result<`UserResult`, `ServerFnError`> - The updated user
///
/// # Errors
/// If the username is empty, return an error
/// If the email is empty, return an error
/// If the password is supplied but don't match, return an error
/// If the user cannot be updated, return an error
/// If the user does not have the required permissions, return an error
#[cfg(feature = "ssr")]
pub async fn update_user_service(
    pool: PgPool,
    user: Option<&User>,
    user_form: UpdateUserForm,
) -> Result<User, ServerFnError> {
    if match user {
        Some(user) => user.username != user_form.original_username,
        None => false,
    } {
        return Err(ServerFnError::new(
            "User does not have the required permissions.",
        ));
    }

    let Some(mut this_user) = User::get_from_username(user_form.original_username, &pool).await
    else {
        return Err(ServerFnError::new("Error while getting user."));
    };
    this_user.username = user_form.username;
    this_user.first_name = user_form.first_name;
    this_user.last_name = user_form.last_name;
    this_user.email = user_form.email;
    this_user.description = user_form.description;

    this_user.update(&pool).await.map_err(|e| {
        let err = format!("Error while updating user: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "ssr")]
    mod ssr_tests {
        use crate::models::test_helpers::create_test_user;

        use super::*;

        #[sqlx::test]
        async fn test_register_user_service(pool: PgPool) {
            let form = RegisterUserForm {
                username: "username".to_string(),
                email: "test@example.com".to_string(),
                password: "password".to_string(),
                password_confirmation: "password".to_string(),
            };
            let user = register_user_service(pool, form.clone()).await.unwrap();
            assert_eq!(user.username, form.username);
            assert_eq!(user.email, form.email);
        }

        #[sqlx::test]
        async fn test_register_user_service_empty_username(pool: PgPool) {
            let form = RegisterUserForm {
                username: String::new(),
                email: "test@example.com".to_string(),
                password: "password".to_string(),
                password_confirmation: "password".to_string(),
            };
            let result = register_user_service(pool, form.clone()).await;
            assert!(result.is_err());
            assert_eq!(
                result.unwrap_err().to_string(),
                "error running server function: Username and password are required."
            );
        }

        #[sqlx::test]
        async fn test_register_user_service_empty_password(pool: PgPool) {
            let form = RegisterUserForm {
                username: "username".to_string(),
                email: "test@example.com".to_string(),
                password: String::new(),
                password_confirmation: String::new(),
            };
            let result = register_user_service(pool, form.clone()).await;
            assert!(result.is_err());
            assert_eq!(
                result.unwrap_err().to_string(),
                "error running server function: Username and password are required."
            );
        }

        #[sqlx::test]
        async fn test_register_user_service_password_mismatch(pool: PgPool) {
            let form = RegisterUserForm {
                username: "username".to_string(),
                email: "test@example.com".to_string(),
                password: "password".to_string(),
                password_confirmation: "password2".to_string(),
            };
            let result = register_user_service(pool, form.clone()).await;
            assert!(result.is_err());
            assert_eq!(
                result.unwrap_err().to_string(),
                "error running server function: Passwords did not match."
            );
        }

        #[sqlx::test]
        async fn test_register_user_service_invalid_email(pool: PgPool) {
            let form = RegisterUserForm {
                username: "username".to_string(),
                email: "invalid-email".to_string(),
                password: "password".to_string(),
                password_confirmation: "password".to_string(),
            };
            let result = register_user_service(pool, form.clone()).await;
            assert!(result.is_err());
            assert_eq!(
                result.unwrap_err().to_string(),
                "error running server function: Email must be valid."
            );
        }

        #[sqlx::test]
        async fn test_update_user_service(pool: PgPool) {
            let (user, _) = create_test_user(&pool, 1).await.unwrap().into_user(None);
            let user_form = UpdateUserForm {
                original_username: user.username.clone(),
                username: "new_username".to_string(),
                email: "new-email@example.com".to_string(),
                description: Some("New description".to_string()),
                first_name: Some("New".to_string()),
                last_name: Some("Name".to_string()),
            };

            let updated_user = update_user_service(pool, Some(&user), user_form)
                .await
                .unwrap();
            assert_eq!(updated_user.username, "new_username");
            assert_eq!(updated_user.email, "new-email@example.com");
            assert_eq!(
                updated_user.description,
                Some("New description".to_string())
            );
            assert_eq!(updated_user.first_name, Some("New".to_string()));
            assert_eq!(updated_user.last_name, Some("Name".to_string()));
        }

        #[sqlx::test]
        async fn test_update_user_service_no_user(pool: PgPool) {
            create_test_user(&pool, 1).await.unwrap().into_user(None);
            let user_form = UpdateUserForm {
                original_username: "other-username".to_string(),
                username: "new_username".to_string(),
                email: "new-email@example.com".to_string(),
                description: Some("New description".to_string()),
                first_name: Some("New".to_string()),
                last_name: Some("Name".to_string()),
            };

            let result = update_user_service(pool, None, user_form).await;
            assert!(result.is_err());
            assert_eq!(
                result.unwrap_err().to_string(),
                "error running server function: Error while getting user."
            );
        }

        #[sqlx::test]
        async fn test_update_user_service_mismatch_username(pool: PgPool) {
            let (user, _) = create_test_user(&pool, 1).await.unwrap().into_user(None);
            let user_form = UpdateUserForm {
                original_username: "other-username".to_string(),
                username: "new_username".to_string(),
                email: "new-email@example.com".to_string(),
                description: Some("New description".to_string()),
                first_name: Some("New".to_string()),
                last_name: Some("Name".to_string()),
            };

            let result = update_user_service(pool, Some(&user), user_form).await;
            assert!(result.is_err());
            assert_eq!(
                result.unwrap_err().to_string(),
                "error running server function: User does not have the required permissions."
            );
        }

        #[sqlx::test]
        async fn test_update_user_service_user_not_found(pool: PgPool) {
            let (mut user, _) = create_test_user(&pool, 1).await.unwrap().into_user(None);
            user.username = "other-username".to_string();
            let user_form = UpdateUserForm {
                original_username: user.username.clone(),
                username: "new_username".to_string(),
                email: "new-email@example.com".to_string(),
                description: Some("New description".to_string()),
                first_name: Some("New".to_string()),
                last_name: Some("Name".to_string()),
            };

            let result = update_user_service(pool, Some(&user), user_form).await;
            assert!(result.is_err());
            assert_eq!(
                result.unwrap_err().to_string(),
                "error running server function: Error while getting user."
            );
        }

        #[sqlx::test]
        async fn test_update_user_service_invalid_form(pool: PgPool) {
            let (user, _) = create_test_user(&pool, 1).await.unwrap().into_user(None);
            let user_form = UpdateUserForm {
                original_username: user.username.clone(),
                username: "new_username".to_string(),
                email: "invalid-email".to_string(),
                description: Some("New description".to_string()),
                first_name: Some("New".to_string()),
                last_name: Some("Name".to_string()),
            };

            let result = update_user_service(pool, Some(&user), user_form).await;
            assert!(result.is_err());
            assert_eq!(
                result.unwrap_err().to_string(),
                "error running server function: Email must be valid."
            );
        }
    }
}
