//! The footer link columns (services, label, legal).
//!
//! The link targets are `Route::PageDetails` variants, so the hrefs are the
//! same `/pages/<slug>` URLs the rest of the site serves.

use dioxus::prelude::*;

use crate::ui::Route;
use crate::ui::use_label;

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
                // The heading is the label's name from the same resource as
                // the navbar brand, so nothing else in the column waits on it.
                h6 {
                    class: "footer-title",
                    SuspenseBoundary {
                        fallback: |_context: SuspenseContext| rsx! {},
                        LabelName {}
                    }
                }
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

/// The footer's label column heading: the record label's name
/// (empty until a label exists).
#[component]
fn LabelName() -> Element {
    let label = use_label()?;
    let name = match label() {
        Ok(result) => result.record_label.name,
        Err(_) => String::new(),
    };
    rsx! { "{name}" }
}
