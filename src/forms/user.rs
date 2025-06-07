//! User-related data structures and forms for registration, updating, and changing passwords.

/// Register a new user form
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct RegisterUserForm {
    /// The username for the new user
    pub username: String,
    /// The email address for the new user
    pub email: String,
    /// The password for the new user
    pub password: String,
    /// The password confirmation for the new user
    /// This should match the password field
    pub password_confirmation: String,
}

/// Update an existing user form
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct UpdateUserForm {
    /// The original username, used to find the user
    pub original_username: String,
    /// The new username for the user
    pub username: String,
    /// The new email address for the user
    pub email: String,
    /// A brief description of the user
    pub description: Option<String>,
    /// The first name of the user
    pub first_name: Option<String>,
    /// The last name of the user
    pub last_name: Option<String>,
}

/// Change password form
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct ChangePasswordForm {
    /// The current password of the user
    pub password: String,
    /// The new password for the user
    pub new_password: String,
    /// The confirmation of the new password
    pub new_password_confirmation: String,
}
