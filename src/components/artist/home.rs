//! The artist home page module

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::{components::A, hooks::use_params_map};
use markdown;

use crate::components::utils::{error::ErrorPage, loading::Loading, status_badge::StatusBadge};
use crate::models::{artist::Artist, release::Release};
use crate::routes::{artist::get_artist, release::get_releases};

/// Renders the artist home page.
#[component]
pub fn ArtistPage() -> impl IntoView {
    let params = use_params_map();
    let artist = RwSignal::new(Artist::default());
    let artist_resource = Resource::new(
        move || params.read().get("slug").unwrap_or_default(),
        get_artist,
    );

    let releases = RwSignal::new(vec![Release::default()]);
    let releases_resource = Resource::new_blocking(
        move || params.read().get("slug").unwrap_or_default(),
        get_releases,
    );
    view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    if let Ok(this_artist) = artist_resource.await {
                        artist.set(this_artist.artist);
                    } else {
                        tracing::error!("Error while getting artist");
                    }
                    if let Ok(release_list) = releases_resource.await {
                        releases.set(release_list.releases);
                    } else {
                        tracing::error!("Error while getting releases for artist");
                    }
                })} <Title text=move || artist.get().name />
                <article class="my-6 md:container md:mx-auto prose">
                    <h1>{move || artist.get().name}</h1>
                    <div class="flex flex-wrap justify-between">
                        <div class="w-1/2">
                            {move || {
                                view! {
                                    <div inner_html=markdown::to_html_with_options(
                                            &artist.get().description,
                                            &markdown::Options::gfm(),
                                        )
                                        .unwrap_or_default() />
                                }
                            }} <div class="flex flex-wrap gap-4 justify-between">
                                <Show when=move || !artist.get().website.is_empty()>
                                    <A
                                        href=move || artist.get().website
                                        attr:class="link link-hover"
                                    >
                                        "🌐 "
                                        {move || artist.get().website}
                                    </A>
                                </Show>
                            </div>
                        </div>
                        <img
                            src=move || artist.get().primary_image_url()
                            alt=move || artist.get().name
                            class="pl-6 w-1/2 h-auto"
                        />
                    </div>

                    <Show when=move || { !releases.get().is_empty() }>
                        <ReleaseList releases=releases artist=artist />
                    </Show>
                </article>
            </ErrorBoundary>
        </Transition>
    }
}

#[component]
/// Fetch and display the list of releases for the artist
pub fn ReleaseList(
    /// The releases to display
    releases: RwSignal<Vec<Release>>,
    /// The artist
    artist: RwSignal<Artist>,
) -> impl IntoView {
    view! {
        <div class="flex flex-wrap gap-4 justify-between">
            <For
                each=move || releases.get()
                key=|release| (release.slug.clone(), release.name.clone())
                let(release)
            >
                <Release release=release artist=artist />
            </For>
        </div>
    }
}

/// Display a single release
/// # Arguments
/// * `release` - The release to display
/// * `artist_slug` - The slug of the artist
/// # Returns
/// * A view of the release
#[component]
pub fn Release(
    /// The release to display
    release: Release,
    /// The artist
    artist: RwSignal<Artist>,
) -> impl IntoView {
    let release = RwSignal::new(release);
    let release_date = move || {
        release.get().release_date.map_or_else(
            || "Unreleased".to_string(),
            |date| date.format("%e %B %Y").to_string(),
        )
    };

    view! {
        <A
            href=move || format!("/artists/{}/{}", artist.get().slug, release.get().slug)
            attr:class="w-1/4 link link-hover min-w-96"
        >
            <div class="shadow-sm not-prose card bg-neutral text-neutral-content">
                <figure class="not-prose">
                    <img
                        src=move || release.get().primary_image_url()
                        alt=move || release.get().name
                    />
                </figure>
                <div class="card-body">
                    <h2 class="card-title">{move || release.get().name}</h2>
                    <p>
                        {release_date}
                        <span class="pl-6">
                            {move || {
                                view! {
                                    <StatusBadge
                                        deleted_at=release.get().deleted_at
                                        published_at=release.get().release_date
                                    />
                                }
                            }}
                        </span>
                    </p>
                </div>
            </div>
        </A>
    }
}
