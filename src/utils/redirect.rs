use leptos_router::{hooks::use_navigate, NavigateOptions};

/// Redirects to a new page.
pub fn redirect(url: &str) {
    let navigate = use_navigate();
    navigate(url, NavigateOptions::default());
}
