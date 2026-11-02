//! The footer link columns (services, label, legal).
//!
//! A dioxus port of the static structure in Leptos
//! `components::record_label::footer`. The link targets are
//! `Route::PageDetails` variants, so the hrefs match the `/pages/<slug>`
//! URLs the Leptos footer hardcodes.

use dioxus::prelude::*;

use crate::ui::Route;
use crate::ui::use_global_state;

/// A footer link to one of the marketing/legal pages.
fn page_link(slug: &str, label: &str) -> Element {
    rsx! {
        Link {
            to: Route::PageDetails {
                slug: slug.to_string(),
            },
            class: "link link-hover",
            "{label}"
        }
    }
}

/// Renders the application footer.
#[component]
pub fn LabelFooter() -> Element {
    let state = use_global_state();
    rsx! {
        footer { class: "p-10 footer bg-neutral text-neutral-content md:footer-horizontal",
            nav {
                h6 { class: "footer-title", "Services" }
                {page_link("branding", "Branding")}
                {page_link("design", "Design")}
                {page_link("marketing", "Marketing")}
                {page_link("advertisment", "Advertisement")}
            }
            nav {
                h6 { class: "footer-title", "{state.read().record_label.name}" }
                {page_link("about-us", "About us")}
                {page_link("contact", "Contact")}
                {page_link("press-kit", "Press kit")}
            }
            nav {
                h6 { class: "footer-title", "Legal" }
                {page_link("terms-of-use", "Terms of use")}
                {page_link("privacy-policy", "Privacy policy")}
                {page_link("cookies", "Cookie policy")}
            }
        }
    }
}
