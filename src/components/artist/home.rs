//! The artist home page module

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::{components::A, hooks::use_params_map};
use markdown;

use crate::components::utils::{error::ErrorPage, loading::Loading, status_badge::StatusBadge};
use crate::models::{
    artist::Artist, music_service::MusicService, release::Release, social_media::SocialMediaService,
};
use crate::routes::{
    artist::get_artist,
    links::{LinksResult, get_links},
    release::get_releases,
};

/// Renders the artist home page.
#[component]
pub fn ArtistPage() -> impl IntoView {
    let params = use_params_map();
    let artist = RwSignal::new(Artist::default());
    let artist_resource = Resource::new(
        move || params.read().get("slug").unwrap_or_default(),
        get_artist,
    );

    let music_links = RwSignal::new(LinksResult::default().music_services);
    let social_media_links = RwSignal::new(LinksResult::default().social_media_services);
    let links_resource = Resource::new_blocking(
        move || params.read().get("slug").unwrap_or_default(),
        get_links,
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
                    if let Ok(links) = links_resource.await {
                        music_links.set(links.music_services);
                        social_media_links.set(links.social_media_services);
                    } else {
                        tracing::error!("Error while getting links for artist");
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
                                    <a
                                        href=move || artist.get().website
                                        class="link link-hover"
                                        target="_blank"
                                    >
                                        "🌐 "
                                        {move || artist.get().website}
                                    </a>
                                </Show>
                            </div> <SocialLinks artist social_media_links />
                            <MusicLinks artist music_links />
                        </div>
                        <div class="w-1/2">
                            <img
                                class="ml-6"
                                src=move || artist.get().primary_image_url()
                                alt=move || artist.get().name
                            />
                        </div>
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
/// Display the social media links for the artist
pub fn SocialLinks(
    /// The artist whose social media links are being displayed
    artist: RwSignal<Artist>,
    /// The social media links to display
    social_media_links: RwSignal<Vec<SocialMediaService>>,
) -> impl IntoView {
    view! {
        <Show when=move || { !social_media_links.get().is_empty() }>
            <div>
                <h2 class="text-2xl">"Follow " {move || artist.get().name}</h2>
                <div class="flex flex-wrap gap-4 not-prose">
                    <For
                        each=move || social_media_links.get()
                        key=|social_media| social_media.platform.clone().to_string()
                        let(social_media)
                    >
                        <a
                            href=move || social_media.url.clone()
                            class="link link-hover"
                            target="_blank"
                        >
                            <div class="w-8 rounded-full">
                                <img
                                    src=format!(
                                        "/images/social_media_services/{}.svg",
                                        social_media.platform.to_string(),
                                    )
                                    alt=social_media.platform.clone().to_string()
                                />
                            </div>
                        </a>
                    </For>
                </div>
            </div>
        </Show>
    }
}

#[component]
/// Display the music links for the artist
pub fn MusicLinks(
    /// The artist whose music links are being displayed
    artist: RwSignal<Artist>,
    /// The music links to display
    music_links: RwSignal<Vec<MusicService>>,
) -> impl IntoView {
    view! {
        <Show when=move || { !music_links.get().is_empty() }>
            <div class="mb-4 md:mx-auto">
                <h2 class="text-2xl">"Listen to " {move || artist.get().name}</h2>
                <div class="flex flex-wrap gap-4 justify-between not-prose">
                    <For
                        each=move || music_links.get()
                        key=|music_service| music_service.platform.clone().to_string()
                        let(music_service)
                    >
                        <a
                            href=move || music_service.url.clone()
                            class="link link-hover"
                            target="_blank"
                        >
                            <div class="shadow-sm card bg-neutral text-neutral-content card-xs">
                                <div class="card-body">
                                    <figure>
                                        <img
                                            class="m-2 w-auto h-6"
                                            src=format!(
                                                "/images/music_services/{}.svg",
                                                music_service.platform.to_string(),
                                            )
                                            alt=music_service.platform.clone().to_string()
                                        />
                                    </figure>
                                </div>
                            </div>
                        </a>
                    </For>
                </div>
            </div>
        </Show>
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
