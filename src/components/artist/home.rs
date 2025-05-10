use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;
use markdown;

use crate::components::utils::{error::ErrorPage, loading::Loading, status_badge::StatusBadge};
use crate::models::{artist::Artist, release::Release};
use crate::routes::{artist::get_artist, release::get_releases};

/// Renders the artist home page.
#[component]
pub fn ArtistPage() -> impl IntoView {
    let params = use_params_map();
    let slug = move || params.read().get("slug");

    let (artist, set_artist) = signal(Artist::default());
    let artist_resource = Resource::new(
        move || artist.get(),
        move |_| slug().map_or_else(|| get_artist(String::new()), get_artist),
    );

    let releases = RwSignal::new(vec![Release::default()]);
    let releases_resource = Resource::new(move || artist.get().slug, get_releases);
    view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    match artist_resource.await {
                        Ok(this_artist) => {
                            *set_artist.write() = this_artist.artist.clone();
                            if let Ok(this_releases) = releases_resource.await {
                                releases.set(this_releases.releases);
                            }
                            this_artist.artist
                        }
                        Err(_) => Artist::default(),
                    };

                    view! {
                        <Title text=artist.get().name />
                        <article class="md:container md:mx-auto prose">
                            <h1>{artist.get().name}</h1>
                            <div class="flex flex-wrap justify-between">
                                <div
                                    inner_html=markdown::to_html_with_options(
                                            &artist.get().description,
                                            &markdown::Options::gfm(),
                                        )
                                        .unwrap_or_default()
                                    class="w-1/2"
                                />
                                <img
                                    src=move || artist.get().primary_image_url()
                                    alt=move || artist.get().name
                                    class="pl-6 w-1/2 h-auto"
                                />
                                {move || {
                                    view! {
                                        <ReleaseList
                                            artist_slug=artist.get().slug
                                            releases=releases
                                        />
                                    }
                                }}
                            </div>
                        </article>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}

#[component]
/// Fetch and display the list of releases for the artist
pub fn ReleaseList(artist_slug: String, releases: RwSignal<Vec<Release>>) -> impl IntoView {
    let artist_slug = RwSignal::new(artist_slug);

    view! {
        {move || {
            let release_rows = releases
                .get()
                .into_iter()
                .map(|release| {
                    view! { <Release release artist_slug /> }
                })
                .collect::<Vec<_>>();
            if release_rows.is_empty() {
                view! { <p>"Coming Soon…"</p> }.into_any()
            } else {
                view! { {release_rows} }.into_any()
            }
        }}
    }
}

/// Display a single release
/// # Arguments
/// * `release` - The release to display
/// * `artist_slug` - The slug of the artist
/// # Returns
/// * A view of the release
#[component]
pub fn Release(#[prop(into)] release: Release, artist_slug: RwSignal<String>) -> impl IntoView {
    let release = RwSignal::new(release);
    let release_date = move || {
        release
            .get()
            .release_date
            .map_or_else(|| "Unreleased".to_string(), |date| date.format("%e %B %Y").to_string())
    };

    view! {
        <a
            href=move || format!("/artists/{}/{}", artist_slug.get(), release.get().slug)
            class="p-6 w-1/4 link link-hover min-w-96"
        >
            <div class="shadow-sm card bg-base-100 bg-neutral text-neutral-content">
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
        </a>
    }
}
