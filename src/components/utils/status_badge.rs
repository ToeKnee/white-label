//! A component to display the status of an item based on its deletion and publication dates.

use leptos::prelude::*;

#[component]
pub fn StatusBadge(
    /// The date when the item was deleted, if applicable.
    #[prop(into)]
    deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    /// The date when the item was published, if applicable. This can also use the `release_date` field for releases.
    published_at: Option<chrono::DateTime<chrono::Utc>>,
) -> impl IntoView {
    let mut badge_text = String::new();
    let mut badge_class = String::new();

    if deleted_at.is_some() {
        badge_text = "Deleted".to_string();
        badge_class = "indicator-item badge badge-error".to_string();
    } else if published_at.is_some() {
        match published_at {
            Some(published_at) if published_at > chrono::Utc::now() => {
                badge_text = "Coming Soon".to_string();
                badge_class = "indicator-item badge badge-info".to_string();
            }
            _ => (),
        }
    } else {
        badge_text = "Draft".to_string();
        badge_class = "indicator-item badge badge-warning".to_string();
    }

    if badge_text.is_empty() {
        view! { "" }.into_any()
    } else {
        view! { <span class=badge_class>{badge_text}</span> }.into_any()
    }
}
