//! Artist pages (MIGRATION.md Phase 4 slice 2), ported from the Leptos
//! `components::artist` group.
//!
//! Covers the artists index and the artist page, plus the artist card
//! (`ArtistList`/`ArtistBox`) shared with the label home page — the Leptos
//! modules each had their own copy.
//!
//! Data arrives through the dioxus resources in `super::data`; each page
//! body runs inside a `SuspenseBoundary`, so SSR waits on the resources and
//! inlines the content (the Leptos `Transition + Suspend` equivalent).
//! Fetch failures keep the Leptos behaviour: log and render with the
//! model's default value rather than an error page.

use dioxus::prelude::*;

use crate::models::artist::Artist;
use crate::models::music_service::MusicService;
use crate::models::release::Release;
use crate::models::social_media::SocialMediaService;
use crate::routes::links::LinksResult;
use crate::ui::Route;
use crate::ui::assets::{bundled_asset, display_image_url};
use crate::ui::data::{get_artist, get_label_artists, get_links, get_releases};
use crate::ui::markdown_html;
use crate::ui::page_data;
use crate::ui::use_label;
use crate::ui::use_page;
use crate::ui::utils::{Loading, StatusBadge};
use crate::utils::shorten_string::shorten_string;

// ---------------------------------------------------------------------------
// Artists index (Leptos `components::artist::list`)
// ---------------------------------------------------------------------------

/// The label's artists, listed as cards.
#[component]
pub fn ArtistsPage() -> Element {
    rsx! {
        SuspenseBoundary {
            fallback: |_context: SuspenseContext| rsx! { Loading {} },
            ArtistsBody {}
        }
    }
}

/// The artists index content, resolved server-side before render.
///
/// Each `use_page` load suspends the boundary in turn — see the note in
/// `super::record_label::LabelHomeBody`.
#[component]
fn ArtistsBody() -> Element {
    let label = use_label()?;
    let label_name = match label() {
        Ok(result) => result.record_label.name,
        Err(error) => {
            tracing::error!("Error while getting label: {error}");
            String::new()
        }
    };
    let artists = use_page(move || page_data(get_label_artists()))?;
    let artists = match artists() {
        Ok(result) => result.artists,
        Err(error) => {
            tracing::error!("Error while getting artists: {error}");
            Vec::new()
        }
    };

    rsx! {
        document::Title { "{label_name}" }
        article { class: "my-6 md:container md:mx-auto prose",
            h1 { "{label_name} Artists" }
            ArtistList { artists }
        }
    }
}

/// Render a list of artists for a record label.
#[component]
pub fn ArtistList(
    /// The artists to display
    artists: Vec<Artist>,
) -> Element {
    rsx! {
        div { class: "flex flex-wrap gap-4 justify-between",
            for artist in artists {
                ArtistBox {
                    key: "{artist.slug}",
                    artist,
                }
            }
        }
    }
}

/// One artist card: image, name, shortened description, status badge.
#[component]
fn ArtistBox(
    /// The artist to display
    artist: Artist,
) -> Element {
    let blurb = shorten_string(artist.description.clone());
    // Held separately so `rsx` can move `artist.name` into the `img` alt
    // while the card title still borrows it.
    let alt = artist.name.clone();
    rsx! {
        Link {
            to: Route::ArtistPage { slug: artist.slug.clone() },
            class: "no-underline",
            div { class: "w-96 shadow-xl not-prose card card-compact bg-neutral text-neutral-content indicator",
                StatusBadge {
                    deleted_at: artist.deleted_at,
                    published_at: artist.published_at,
                }
                figure {
                    img { src: display_image_url(&artist.primary_image_url()), alt }
                }
                div { class: "card-body",
                    h2 { class: "card-title", "{artist.name}" }
                    p { "{blurb}" }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Artist page (Leptos `components::artist::home`)
// ---------------------------------------------------------------------------

/// One artist's public page: bio, links, and their releases.
#[component]
pub fn ArtistPage(
    /// The artist slug from the URL.
    slug: String,
) -> Element {
    rsx! {
        SuspenseBoundary {
            fallback: |_context: SuspenseContext| rsx! { Loading {} },
            ArtistBody { slug }
        }
    }
}

/// The artist page content: the artist, links and releases loads each
/// suspend the boundary in turn — see the note in
/// `super::record_label::LabelHomeBody`.
#[component]
fn ArtistBody(slug: String) -> Element {
    let links_slug = slug.clone();
    let releases_slug = slug.clone();
    // Cloning inside the closures: `use_page` keeps them as `FnMut`s it
    // may call again on a later refetch.
    let artist = use_page(move || page_data(get_artist(slug.clone())))?;
    let artist = match artist() {
        Ok(result) => result.artist,
        Err(error) => {
            tracing::error!("Error while getting artist: {error}");
            Artist::default()
        }
    };
    let links = use_page(move || page_data(get_links(links_slug.clone())))?;
    let links = match links() {
        Ok(result) => result,
        Err(error) => {
            tracing::error!("Error while getting links for artist: {error}");
            LinksResult::default()
        }
    };
    let releases = use_page(move || page_data(get_releases(releases_slug.clone())))?;
    let releases = match releases() {
        Ok(result) => result.releases,
        Err(error) => {
            tracing::error!("Error while getting releases for artist: {error}");
            Vec::new()
        }
    };

    rsx! {
        document::Title { "{artist.name}" }
        article { class: "my-6 md:container md:mx-auto prose",
            h1 { "{artist.name}" }
            div { class: "flex flex-wrap justify-between",
                div { class: "w-1/2",
                    div { dangerous_inner_html: markdown_html(&artist.description) }
                    div { class: "flex flex-wrap gap-4 justify-between",
                        if !artist.website.is_empty() {
                            a {
                                href: artist.website.clone(),
                                class: "link link-hover",
                                target: "_blank",
                                "🌐 {artist.website}"
                            }
                        }
                    }
                    SocialLinks {
                        artist_name: artist.name.clone(),
                        social_media_links: links.social_media_services,
                    }
                    MusicLinks {
                        artist_name: artist.name.clone(),
                        music_links: links.music_services,
                    }
                }
                div { class: "w-1/2",
                    img {
                        class: "ml-6",
                        src: display_image_url(&artist.primary_image_url()),
                        alt: artist.name.clone(),
                    }
                }
            }
            if !releases.is_empty() {
                ReleaseList {
                    releases,
                    artist_slug: artist.slug.clone(),
                }
            }
        }
    }
}

/// The social media links for an artist, as icon links.
#[component]
fn SocialLinks(
    /// The artist's name, for the heading.
    artist_name: String,
    /// The social media links to display
    social_media_links: Vec<SocialMediaService>,
) -> Element {
    if social_media_links.is_empty() {
        return rsx! {};
    }
    rsx! {
        div {
            h2 { class: "text-2xl", "Follow {artist_name}" }
            div { class: "flex flex-wrap gap-4 not-prose",
                for social_media in social_media_links {
                    a {
                        key: "{social_media.platform}",
                        href: social_media.url.clone(),
                        class: "link link-hover",
                        target: "_blank",
                        div { class: "w-8 rounded-full",
                            img {
                                src: bundled_asset(&format!(
                                    "/images/social_media_services/{platform}.svg",
                                    platform = social_media.platform,
                                )),
                                alt: social_media.platform.to_string(),
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The music service links for an artist, as logo cards.
#[component]
fn MusicLinks(
    /// The artist's name, for the heading.
    artist_name: String,
    /// The music service links to display
    music_links: Vec<MusicService>,
) -> Element {
    if music_links.is_empty() {
        return rsx! {};
    }
    rsx! {
        div { class: "mb-4 md:mx-auto",
            h2 { class: "text-2xl", "Listen to {artist_name}" }
            div { class: "flex flex-wrap gap-4 justify-between not-prose",
                for music_service in music_links {
                    a {
                        key: "{music_service.platform}",
                        href: music_service.url.clone(),
                        class: "link link-hover",
                        target: "_blank",
                        div { class: "shadow-sm card bg-neutral text-neutral-content card-xs",
                            div { class: "card-body",
                                figure {
                                    img {
                                        class: "m-2 w-auto h-6",
                                        src: bundled_asset(&format!(
                                            "/images/music_services/{platform}.svg",
                                            platform = music_service.platform,
                                        )),
                                        alt: music_service.platform.to_string(),
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The releases for an artist, as cards.
#[component]
fn ReleaseList(
    /// The releases to display
    releases: Vec<Release>,
    /// The slug of the artist the releases belong to, for the card links.
    artist_slug: String,
) -> Element {
    rsx! {
        div { class: "flex flex-wrap gap-4 justify-between",
            for release in releases {
                ReleaseCard {
                    key: "{release.slug}",
                    release,
                    artist_slug: artist_slug.clone(),
                }
            }
        }
    }
}

/// One release card: cover, name, release date, status badge.
#[component]
fn ReleaseCard(
    /// The release to display
    release: Release,
    /// The slug of the artist, used to build the public release URL.
    artist_slug: String,
) -> Element {
    // The Leptos original printed "Unreleased" when there is no date.
    let release_date = release.release_date.map_or_else(
        || "Unreleased".to_string(),
        |date| date.format("%e %B %Y").to_string(),
    );
    // Held separately so `rsx` can move `release.name` into the `img` alt
    // while the card title still borrows it.
    let alt = release.name.clone();
    rsx! {
        Link {
            to: Route::ReleasePage {
                artist_slug,
                release_slug: release.slug.clone(),
            },
            class: "w-1/4 link link-hover min-w-96",
            div { class: "shadow-sm not-prose card bg-neutral text-neutral-content",
                figure { class: "not-prose",
                    img {
                        src: display_image_url(&release.primary_image_url()),
                        alt,
                    }
                }
                div { class: "card-body",
                    h2 { class: "card-title", "{release.name}" }
                    p {
                        "{release_date}"
                        span { class: "pl-6",
                            StatusBadge {
                                deleted_at: release.deleted_at,
                                published_at: release.release_date,
                            }
                        }
                    }
                }
            }
        }
    }
}
