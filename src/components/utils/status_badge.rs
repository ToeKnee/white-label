use leptos::prelude::*;

#[component]
pub fn StatusBadge(
    #[prop(into)] deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    published_at: Option<chrono::DateTime<chrono::Utc>>,
) -> impl IntoView {
    let mut badge_text = String::new();
    let mut badge_class = String::new();

    if deleted_at.is_some() {
        badge_text = "Deleted".to_string();
        badge_class = "indicator-item badge badge-error".to_string();
    } else if published_at.is_some() {
        if published_at.unwrap() > chrono::Utc::now() {
            badge_text = "Scheduled".to_string();
            badge_class = "indicator-item badge badge-info".to_string();
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
