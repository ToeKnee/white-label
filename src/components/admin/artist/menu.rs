//! Aritst admin menu component.

use leptos::prelude::*;

/// The artist admin menu options.
#[derive(Clone, Eq, PartialEq)]
pub enum Page {
    /// The artist profile.
    Profile,
    /// The artist releases.
    Releases,
}

fn classes(selected: &Page, current_menu_item: &Page) -> String {
    if selected == current_menu_item {
        "link link-hover active"
    } else {
        "link link-hover"
    }
    .to_string()
}

/// Renders the artist admin menu.
///
/// Arguments:
/// * `slug` - The slug of the artist.
/// * `selected` - The currently selected menu item.
#[component]
pub fn Menu<'a>(
    /// The slug of the artist.
    slug: RwSignal<std::string::String>,
    /// The currently selected menu item.
    selected: &'a Page,
) -> impl IntoView {
    view! {
        <ul class="menu menu-horizontal bg-base-200">
            <li class="menu-active">
                <a
                    class=classes(selected, &Page::Profile)
                    href=move || format!("/admin/artist/{}", slug.get())
                >
                    Profile
                </a>

            </li>
            <li>
                <a
                    class=classes(selected, &Page::Releases)
                    href=move || format!("/admin/artist/{}/releases", slug.get())
                >
                    Releases
                </a>
            </li>
        </ul>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classes() {
        assert_eq!(
            classes(&Page::Profile, &Page::Profile),
            "link link-hover active"
        );
        assert_eq!(classes(&Page::Profile, &Page::Releases), "link link-hover");
        assert_eq!(classes(&Page::Releases, &Page::Profile), "link link-hover");
        assert_eq!(
            classes(&Page::Releases, &Page::Releases),
            "link link-hover active"
        );
    }
}
