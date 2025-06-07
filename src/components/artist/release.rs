//! This displays a release for an artist.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;
use markdown;

use crate::components::utils::{error::ErrorPage, loading::Loading};
use crate::models::{artist::Artist, release::Release, track_with_artists::TrackWithArtists};
use crate::routes::{artist::get_artist, release::get_release};
use crate::utils::{redirect::redirect, shorten_string::shorten_string};

/// Renders the release page.
#[component]
pub fn ReleasePage() -> impl IntoView {
    let params = use_params_map();
    let artist_slug = RwSignal::new(String::new());
    let release_slug = RwSignal::new(String::new());
    Effect::new_isomorphic(move || {
        artist_slug.set(params.read().get("artist_slug").unwrap_or_default());
        release_slug.set(params.read().get("release_slug").unwrap_or_default());
    });

    let artist = RwSignal::new(Artist::default());
    let artist_resource = Resource::new(
        move || artist_slug,
        move |artist_slug| get_artist(artist_slug.get()),
    );

    let release = RwSignal::new(Release::default());
    let artists = RwSignal::new(Vec::new()); // Artists on the release
    let tracks = RwSignal::new(Vec::new()); // Tracks on the release
    let release_resource = Resource::new(
        move || [artist_slug, release_slug],
        move |[artist_slug, release_slug]| get_release(artist_slug.get(), release_slug.get()),
    );

    view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    if let Ok(this_artist) = artist_resource.await {
                        if !this_artist.artist.slug.is_empty() {
                            artist.set(this_artist.artist);
                        }
                    } else {
                        tracing::error!("Error while getting artist");
                        redirect("/artists");
                    }
                    if let Ok(this_release) = release_resource.await {
                        if !this_release.release.slug.is_empty() {
                            release.set(this_release.release.clone());
                            artists.set(this_release.artists);
                            tracks.set(this_release.tracks);
                        }
                    } else {
                        tracing::error!("Error while getting release");
                        redirect(&format!("/artists/{}", artist.get().slug));
                    }

                    view! {
                        <Title text=release.get().name />
                        <article class="md:container md:mx-auto prose">
                            <h1>{release.get().name}</h1>
                            <div class="flex flex-wrap justify-between">
                                <div
                                    inner_html=markdown::to_html_with_options(
                                            &release.get().description,
                                            &markdown::Options::gfm(),
                                        )
                                        .unwrap_or_default()
                                    class="w-1/2"
                                />
                                <img
                                    src=move || release.get().primary_image_url()
                                    alt=move || release.get().name
                                    class="pl-6 w-1/2 h-auto"
                                />
                            </div>

                            {move || {
                                view! { <TrackList tracks /> }
                            }}
                        </article>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}

#[component]
/// Fetch and display the list of tracks for this release.
pub fn TrackList(
    /// The tracks to display
    tracks: RwSignal<Vec<TrackWithArtists>>,
) -> impl IntoView {
    move || {
        let track_tows = tracks
            .get()
            .into_iter()
            .map(|track| {
                view! { <Track track /> }
            })
            .collect::<Vec<_>>();
        if track_tows.is_empty() {
            view! { "" }.into_any()
        } else {
            view! {
                <h2>Tracks</h2>
                <ul class="list bg-base-100">{track_tows}</ul>
            }
            .into_any()
        }
    }
}

/// Display a single release
/// # Arguments
/// * `release` - The release to display
/// * `artist_slug` - The slug of the artist
/// # Returns
/// * A view of the release
#[component]
pub fn Track(
    /// The track to display
    #[prop(into)]
    track: TrackWithArtists,
) -> impl IntoView {
    let track = RwSignal::new(track);

    view! {
        <li class="list-row">
            <div>
                <img
                    class="not-prose size-10"
                    src=move || track.get().track.primary_image_url()
                    alt=move || track.get().track.name
                />
            </div>
            <div>
                <div>{move || track.get().track.name}</div>
                <div class="text-xs font-semibold uppercase opacity-60">
                    {move || view! { <FeaturedTrackArtists track=track.get() /> }}
                </div>
                <p class="text-xs list-col-wrap">
                    {move || shorten_string(track.get().track.description)}
                </p>
            </div>

        // <button class="btn btn-square btn-ghost">
        // <svg class="size-[1.2em]" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
        // <g
        // stroke-linejoin="round"
        // stroke-linecap="round"
        // stroke-width="2"
        // fill="none"
        // stroke="currentColor"
        // >
        // <path d="M6 3L20 12 6 21 6 3z"></path>
        // </g>
        // </svg>
        // </button>
        // <button class="btn btn-square btn-ghost">
        // <svg class="size-[1.2em]" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
        // <g
        // stroke-linejoin="round"
        // stroke-linecap="round"
        // stroke-width="2"
        // fill="none"
        // stroke="currentColor"
        // >
        // <path d="M19 14c1.49-1.46 3-3.21 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.76 0-3 .5-4.5 2-1.5-1.5-2.74-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4.05 3 5.5l7 7Z"></path>
        // </g>
        // </svg>
        // </button>
        </li>
    }
}

/// Names of artists who have contributed to the track, but not the primary artist.
/// # Arguments
/// * `track` - The track to display featured artists for
/// # Returns
/// * A view of the featured artists
#[component]
#[allow(clippy::needless_pass_by_value)]
pub fn FeaturedTrackArtists(
    /// The track to display featured artists for
    #[prop(into)]
    track: TrackWithArtists,
) -> impl IntoView {
    let primary_artist_id = track.track.primary_artist_id;
    let featured_artists = {
        track
            .artists
            .iter()
            .filter(|artist| artist.id != primary_artist_id)
            .map(|artist| artist.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    };
    if featured_artists.is_empty() {
        return view! { "" }.into_any();
    }
    view! {
        <span class="text-xs font-semibold uppercase opacity-60">
            "Featuring " {featured_artists}
        </span>
    }
    .into_any()
}
