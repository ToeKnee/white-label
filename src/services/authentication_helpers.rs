use leptos::logging;
use leptos::prelude::ServerFnError;

use crate::models::auth::User;

/// Return a user with required permissions
///
/// # Arguments
/// user: Option<&User> - The user to check permissions for
/// permissions: Vec<&str> - The permissions required
///
/// # Returns
/// Result<User, `ServerFnError`> - The user with the required permissions
///
/// # Errors
/// If the user is not authenticated, return an error
/// If the user does not have the required permissions, return an error
/// If the user is not supplied, return an error
pub fn user_with_permissions(
    user: Option<&User>,
    permissions: Vec<&str>,
) -> Result<User, ServerFnError> {
    let Some(user) = user else {
        logging::error!("User not supplied");
        return Err(ServerFnError::new("User not supplied."));
    };
    // Check if the user is authenticated
    // If the user is not authenticated, return an error
    if !user.is_authenticated() {
        return Err(ServerFnError::new(
            "You must be logged in to view this page.",
        ));
    }

    // Check if the user has the required permissions
    // If the user does not have the required permissions, return an error
    for permission in permissions {
        if !user.permissions.contains(permission) {
            logging::error!("User does not have the required permission {permission}");
            return Err(ServerFnError::new("You do not have permission."));
        }
    }

    Ok(user.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{auth::User, test_helpers::create_test_user_with_permissions};
    use leptos::prelude::ServerFnError;
    use sqlx::PgPool;

    #[sqlx::test]
    fn test_user_with_permissions(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, vec!["admin", "label_owner"])
            .await
            .unwrap();

        let result = user_with_permissions(Some(&user), permissions);
        assert_eq!(result, Ok(user));
    }

    #[test]
    fn test_user_with_permissions_no_user() {
        let permissions = vec![];

        let result = user_with_permissions(None, permissions);
        assert_eq!(result, Err(ServerFnError::new("User not supplied.")));
    }

    #[test]
    fn test_user_with_permissions_not_authenticated() {
        let user = User::default();
        let permissions = vec!["admin", "label_owner"];

        let result = user_with_permissions(Some(&user), permissions);
        assert_eq!(
            result,
            Err(ServerFnError::new(
                "You must be logged in to view this page."
            ))
        );
    }

    #[sqlx::test]
    fn test_user_with_permissions_no_permissions(pool: PgPool) {
        let permissions = vec![];
        let user = create_test_user_with_permissions(&pool, 1, vec![])
            .await
            .unwrap();

        let result = user_with_permissions(Some(&user), permissions);
        assert_eq!(result, Ok(user));
    }

    #[sqlx::test]
    fn test_user_with_permissions_no_permission(pool: PgPool) {
        let user = create_test_user_with_permissions(&pool, 1, vec![])
            .await
            .unwrap();
        let permissions = vec!["admin", "label_owner"];

        let result = user_with_permissions(Some(&user), permissions);
        assert_eq!(
            result,
            Err(ServerFnError::new("You do not have permission."))
        );
    }
}
