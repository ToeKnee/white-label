use leptos::prelude::*;

use crate::app::UserContext;
use crate::utils::redirect::redirect;

/// User is authenticated.
/// Does not check for permissions.
/// Redirects to the given path if the user is not authenticated.
pub fn authenticated_or_redirect(redirect_path: &str) {
    let user_context = expect_context::<UserContext>();

    if !user_context.0.get().is_active() {
        redirect(redirect_path);
    }
}

/// Permission check for the user.
pub fn permission_or_redirect(permission: &str, redirect_path: &str) {
    let user_context = expect_context::<UserContext>();

    if !(user_context.0.get().is_active() || user_context.0.get().permissions.contains(permission)) {
        redirect(redirect_path);
    }
}
