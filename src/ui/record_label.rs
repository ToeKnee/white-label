//! The record label home page (MIGRATION.md Phase 4 slice 2), ported from
//! the Leptos `components::record_label::home` module.
//!
//! The label itself comes from the shared resource [`App`] provides
//! (`super::use_label`); the artist cards reuse
//! [`crate::ui::artist::ArtistList`] (one copy where Leptos had two).

use dioxus::prelude::*;

use crate::ui::artist::ArtistList;
use crate::ui::data::get_label_artists;
use crate::ui::markdown_html;
use crate::ui::use_label;
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
/// Resource scheduling rule (learned the hard way in slice 2): create
/// each resource and `suspend()` on it immediately, inside the boundary —
/// the `SuspenseBoundary` then drives the server fn's future itself.
/// Resources spawned early and awaited after another suspend are never
/// polled, hanging the render.
#[component]
fn LabelHomeBody() -> Element {
    // A failed label load renders the empty defaults, like the Leptos
    // store starting at `RecordLabel::default()` (the h1 is then blank
    // and the artist fetch below fails the way the Leptos skip did).
    let label = use_label().suspend()?;
    let label = match label() {
        Ok(result) => result.record_label,
        Err(error) => {
            tracing::error!("Error while getting label: {error}");
            crate::models::record_label::RecordLabel::default()
        }
    };
    let artists = use_resource(get_label_artists);
    let artists = artists.suspend()?;
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
