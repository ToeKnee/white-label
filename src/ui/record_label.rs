//! The record label home page.
//!
//! The label itself comes from the shared resource [`App`] provides
//! (`super::use_label`); the artist cards reuse
//! [`crate::ui::artist::ArtistList`].

use dioxus::prelude::*;

use crate::ui::artist::ArtistList;
use crate::ui::data::get_label_artists;
use crate::ui::markdown_html;
use crate::ui::page_data;
use crate::ui::use_label;
use crate::ui::use_page;
use crate::ui::utils::Loading;

/// Landing page: the label's name and description, then its artists.
#[component]
pub fn RecordLabelHome() -> Element {
    rsx! {
        SuspenseBoundary {
            fallback: |_context: SuspenseContext| rsx! { Loading {} },
            LabelHomeBody {}
        }
    }
}

/// The home page content, resolved server-side before render.
///
/// `use_page`'s `?` suspends the enclosing boundary until that load
/// answers, so the body always reads finished values and dependent loads
/// can simply use the value above them.
#[component]
fn LabelHomeBody() -> Element {
    // A failed label load renders the empty defaults (the h1 is then blank
    // and the artist fetch below fails too).
    let label = use_label()?;
    let label = match label() {
        Ok(result) => result.record_label,
        Err(error) => {
            tracing::error!("Error while getting label: {error}");
            crate::models::record_label::RecordLabel::default()
        }
    };
    // Cloning inside the closure: `use_page` keeps it as an `FnMut` it may
    // call again on a later refetch. Deleted rows are never shown on the
    // public side (`false` is fixed here; the toggle is admin-only chrome).
    let artists = use_page(move || page_data(get_label_artists(false)))?;
    let artists = match artists() {
        Ok(result) => result.artists,
        Err(error) => {
            tracing::error!("Error while getting artists: {error}");
            Vec::new()
        }
    };

    rsx! {
        document::Title { "{label.name}" }
        article { class: "my-6 md:container md:mx-auto prose",
            h1 { "{label.name}" }
            div { dangerous_inner_html: markdown_html(&label.description) }
            h2 { "Artists" }
            ArtistList { artists }
        }
    }
}
