//! Upload or replace an artist's images (MIGRATION.md Phase 5 slice 5g),
//! ported from the Leptos `components::admin::artist::images`.
//!
//! The page is the artist tabs' Images entry: the picker goes to
//! [`UploadConfiguration::Artist`] (its directory, mime types, 100 MB limit
//! and `admin` + `label_owner` requirement), addressed by the artist slug,
//! and the artist's current primary image is shown underneath.
//!
//! Deviations from the Leptos original:
//!
//! - The `<img>` follows a finished upload at once: the upload answers with
//!   the stored name and `on_uploaded` points the image at
//!   [`stored_file_url`]. Leptos left the previous file on screen until the
//!   page was reloaded (its widget logged a TODO about exactly that).
//! - The Leptos page read the artist from its `GlobalState` store; the port
//!   fetches it through the `get_artist` twin, so a missing artist gets the
//!   not-found page as in the other editors.

use dioxus::prelude::*;

use crate::config::upload::UploadConfiguration;
use crate::ui::assets::display_image_url;
use crate::ui::data::get_artist;
use crate::ui::upload::{FileUploadWithProgress, stored_file_url};
use crate::ui::utils::Loading;
use crate::ui::utils::permission_allows;
use crate::ui::{Route, page_data, redirect, use_page, use_user};

/// Upload/replace an artist's images (Leptos `artist/images.rs`).
#[component]
pub fn EditArtistImages(
    /// The artist slug from the URL.
    artist_slug: String,
) -> Element {
    rsx! {
        SuspenseBoundary {
            fallback: |_context: SuspenseContext| rsx! { Loading {} },
            EditArtistImagesBody { artist_slug }
        }
    }
}

#[component]
fn EditArtistImagesBody(artist_slug: String) -> Element {
    // The upload configuration carries the permissions itself, but the page
    // is gated like the other artist editors (`label_owner`), so an
    // `admin`-only session never sees a picker it cannot use.
    let session = use_user()?;
    let user = session().ok().flatten();
    if !permission_allows(user.as_ref(), "label_owner") {
        redirect(Route::Dashboard {});
        return rsx! { Loading {} };
    }

    // The name carries the header and the image alt text.
    let images_artist_slug = artist_slug.clone();
    let artist_resource = use_page(move || page_data(get_artist(images_artist_slug.clone())))?;
    let artist = match artist_resource() {
        Ok(result) => result.artist,
        Err(error) => {
            tracing::error!(
                "Error while getting artist for the images editor (slug {artist_slug}): {error}"
            );
            redirect(Route::NotFound {
                segments: vec!["admin".into(), "artist-not-found".into()],
            });
            return rsx! { Loading {} };
        }
    };
    // Read the current URL before the name moves into the header.
    let current_image_url = display_image_url(&artist.primary_image_url());
    let artist_name = artist.name;

    // Set once an upload lands; the image below then shows the new file
    // without a reload. Reading the signal in the body (rather than in the
    // rsx block) is what re-renders this body when an upload finishes.
    let mut uploaded_image = use_signal(|| None::<String>);
    let shown_image = uploaded_image().map_or(current_image_url, |stored| {
        stored_file_url(UploadConfiguration::Artist, &stored)
    });
    let upload_slug = artist_slug;
    let image_alt = artist_name.clone();

    rsx! {
        document::Title { "{artist_name} Images" }
        h1 { "{artist_name} Images" }
        div { class: "grid gap-6",
            div { class: "divider", "Images" }
            FileUploadWithProgress {
                config: UploadConfiguration::Artist,
                slug: upload_slug,
                on_uploaded: move |stored_name: String| uploaded_image.set(Some(stored_name)),
            }
            img {
                src: shown_image,
                alt: image_alt,
            }
        }
    }
}
