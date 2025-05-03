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
    view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    match artist_resource.await {
                        Ok(this_artist) => {
                            *set_artist.write() = this_artist.artist.clone();
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
                                <ReleaseList artist_slug=artist.get().slug />
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
pub fn ReleaseList(artist_slug: String) -> impl IntoView {
    let artist_slug = RwSignal::new(artist_slug);
    let releases = RwSignal::new(vec![Release::default()]);
    let releases_resource = Resource::new(move || artist_slug.get(), get_releases);

    view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    if let Ok(this_releases) = releases_resource.await {
                        releases.set(this_releases.releases);
                    }
                    let release_rows = releases
                        .get()
                        .into_iter()
                        .map(|release| {
                            view! { <Release release artist_slug=artist_slug /> }
                        })
                        .collect::<Vec<_>>();
                    view! {
                        {if release_rows.is_empty() {
                            view! { <p>"Coming Soon…"</p> }.into_any()
                        } else {
                            view! { {release_rows} }.into_any()
                        }}
                    }
                })}
            </ErrorBoundary>
        </Transition>
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
    let release_date = release
        .release_date
        .map_or_else(|| "Unreleased".to_string(), |date| date.format("%e %B %Y").to_string());

    view! {
        <a
            href=format!("/artists/{}/{}", artist_slug.get(), release.slug)
            class="p-6 w-1/4 link link-hover min-w-96"
        >
            <div class="shadow-sm card bg-base-100 bg-neutral text-neutral-content">
                <figure class="m-0">
                    <img src=release.primary_image_url() alt=release.name.clone() />
                </figure>
                <div class="m-0 card-body">
                    <h2 class="m-0 card-title">{release.name}</h2>
                    <p class="m-0">
                        {release_date} <span class="pl-6">
                            <StatusBadge
                                deleted_at=release.deleted_at
                                published_at=release.release_date
                            />
                        </span>
                    </p>
                </div>
            </div>
        </a>
    }
}
