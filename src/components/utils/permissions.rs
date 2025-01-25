use leptos::prelude::*;

use crate::app::UserContext;
use crate::utils::redirect::redirect;

/// Permission check for the user.
pub fn permission_or_redirect(permission: &str, redirect_path: &str) {
    let user_context = expect_context::<UserContext>();
    let (user, _set_user) = signal(user_context.0.get());

    if !(user.get().is_active() || user.get().permissions.contains(permission)) {
        redirect(redirect_path);
    }
}
