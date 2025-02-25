use leptos_router::{NavigateOptions, hooks::use_navigate};

/// Redirects to a new page.
pub fn redirect(url: &str) {
    let navigate = use_navigate();
    navigate(url, NavigateOptions::default());
}
