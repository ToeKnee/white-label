#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct RegisterUserForm {
    pub username: String,
    pub email: String,
    pub password: String,
    pub password_confirmation: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct UpdateUserForm {
    // The original username, used to find the user
    pub original_username: String,
    pub username: String,
    pub email: String,
    pub description: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct PasswordChangeForm {
    pub password: String,
    pub password_confirmation: String,
}
