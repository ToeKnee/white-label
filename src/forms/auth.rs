//! Forms for the reactive auth endpoints in `crate::ui::auth`.
//!
//! The field names match the login/register markup, so the same inputs feed
//! the javascript-off handlers (`crate::server::forms`) and the reactive
//! twins, and both paths see the same validation.

/// The login form's fields.
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
#[serde(default)]
pub struct ClientLoginForm {
    /// The username to log in as.
    pub username: String,
    /// The password to check against the stored hash.
    pub password: String,
    /// Any value at all when the "Remember me" box is ticked (the browser
    /// omits an unchecked box, exactly as in the form-encoded post).
    pub remember: Option<String>,
}

/// The register form's fields.
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
#[serde(default)]
pub struct ClientRegisterForm {
    /// The username for the new account.
    pub username: String,
    /// The email address for the new account.
    pub email: String,
    /// The password for the new account.
    pub password: String,
    /// Must match `password`.
    pub password_confirmation: String,
    /// Any value at all when "Remember me" is ticked.
    pub remember: Option<String>,
}
