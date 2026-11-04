//! Marketing/legal page details (MIGRATION.md Phase 4 slice 2), ported
//! from the Leptos `components::page` module.
//!
//! `label_owner` sessions can also see unpublished pages (service-side
//! visibility rule, same as the Leptos `get_page` route).

use dioxus::prelude::*;

use crate::models::page::Page;
use crate::ui::data::get_page;
use crate::ui::markdown_html;
use crate::ui::page_data;
use crate::ui::use_page;
use crate::ui::utils::Loading;

/// One page by slug: title plus markdown body.
#[component]
pub fn PageDetails(
    /// The page slug from the URL.
    slug: String,
) -> Element {
    rsx! {
        SuspenseBoundary {
            fallback: |_context: SuspenseContext| rsx! { Loading {} },
            PageBody { slug }
        }
    }
}

/// The page content, resolved server-side before render.
#[component]
fn PageBody(slug: String) -> Element {
    let page = use_page(move || page_data(get_page(slug.clone())))?;

    // Failed loads render the empty default, as the Leptos component did
    // (the route underneath answers 404, so this page is still served).
    let page = match page() {
        Ok(result) => result.page,
        Err(error) => {
            tracing::error!("Error while getting page: {error}");
            Page::default()
        }
    };

    rsx! {
        document::Title { "{page.name}" }
        article { class: "my-6 md:container md:mx-auto prose",
            h1 { "{page.name}" }
            div { dangerous_inner_html: markdown_html(&page.body) }
        }
    }
}
