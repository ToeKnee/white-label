//! Posted fields for the auth forms.
//!
//! The field names match the login/register/profile/change-password markup, so
//! the same inputs feed the javascript-off handlers (`crate::server::forms`)
//! and the reactive twins (`crate::ui::auth`), and both paths see the same
//! validation. The profile and change-password fields turn themselves into the
//! service forms their handler and twin call, so neither path invents a value
//! the other would not.

use crate::forms::user::{ChangePasswordForm, UpdateUserForm};

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

/// The profile form's posted fields, exactly as the browser posts them.
///
/// Unlike a login form, every input is pre-filled from the account, so the
/// optional text inputs always arrive — blank when the field is empty.
/// `into_service_form` turns a blank one back into "no value".
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ClientProfileFields {
    /// The account being edited, cross-checked against the session.
    pub original_username: String,
    /// The (possibly new) username.
    pub username: String,
    /// The (possibly new) email address.
    pub email: String,
    /// The bio; blank means no bio.
    pub description: String,
    /// The given name; blank means none.
    pub first_name: String,
    /// The family name; blank means none.
    pub last_name: String,
}

impl ClientProfileFields {
    /// The service form these posts turn into, shared by the javascript-off
    /// handler and the reactive twin.
    #[must_use]
    pub fn into_service_form(self) -> UpdateUserForm {
        UpdateUserForm {
            original_username: self.original_username,
            username: self.username,
            email: self.email,
            description: blank_is_none(self.description),
            first_name: blank_is_none(self.first_name),
            last_name: blank_is_none(self.last_name),
        }
    }
}

/// The change-password form's posted fields.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ClientPasswordFields {
    /// The current password, checked against the stored hash.
    pub password: String,
    /// The new password.
    pub new_password: String,
    /// Must match `new_password`.
    pub new_password_confirmation: String,
}

impl ClientPasswordFields {
    /// The service form these posts turn into.
    #[must_use]
    pub fn into_service_form(self) -> ChangePasswordForm {
        ChangePasswordForm {
            password: self.password,
            new_password: self.new_password,
            new_password_confirmation: self.new_password_confirmation,
        }
    }
}

/// A blank text input means "no value", the way the service forms express
/// optional text.
fn blank_is_none(text: String) -> Option<String> {
    (!text.is_empty()).then_some(text)
}
