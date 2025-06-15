//! Admin menu module

use leptos::prelude::*;
use reactive_stores::Store;

use crate::components::utils::{error::ErrorPage, loading::Loading};
use crate::models::{artist::Artist, record_label::RecordLabel, release::Release};
use crate::routes::{
    record_label::{get_label_artists, get_record_label},
    release::get_releases,
};
use crate::store::{GlobalState, GlobalStateStoreFields};

/// Admin menu component that provides navigation links for the admin section of the application.
#[component]
pub fn AdminMenu() -> impl IntoView {
    let store = expect_context::<Store<GlobalState>>();
    let record_label_resource = Resource::new(move || {}, |()| get_record_label());

    view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    match record_label_resource.await {
                        Ok(record_label_result) => {
                            store.record_label().set(record_label_result.record_label);
                        }
                        Err(e) => {
                            tracing::error!("Error: {e:?}");
                        }
                    }
                    view! {
                        <ul class="w-56 menu bg-base-200 rounded-box">
                            <li>
                                <a href="/admin">"Dashboard"</a>
                            </li>
                            <li>
                                <a href="/admin/label">{move || store.record_label().get().name}</a>
                            </li>
                            <li>
                                <ArtistsMenu record_label=store.record_label().get() />
                            </li>

                            <li>
                                <a href="/admin/pages">"Pages"</a>
                            </li>
                        </ul>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}

#[component]
fn ArtistsMenu(record_label: RecordLabel) -> impl IntoView {
    let artists_resource = Resource::new(move || record_label.id, get_label_artists);
    let artists = RwSignal::new(Vec::new());

    view! {
        <Transition fallback=move || {
            view! { "Loading..." }
        }>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    if let Ok(artists_result) = artists_resource.await {
                        artists.set(artists_result.artists);
                    }
                    view! {
                        <details open>
                            <summary>
                                <a href="/admin/artists">Artists</a>
                            </summary>
                            <ul>
                                <Show
                                    when=move || { !artists.get().is_empty() }
                                    fallback=|| view! { <li>"No artists yet..."</li> }
                                >
                                    <For
                                        each=move || artists.get()
                                        key=|artist| (artist.slug.clone(), artist.name.clone())
                                        let(artist)
                                    >
                                        <li>
                                            <ArtistMenuRow artist />
                                        </li>
                                    </For>
                                </Show>
                                <li>
                                    <a href="/admin/artist">"Create Artist"</a>
                                </li>
                            </ul>
                        </details>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}

#[component]
fn ArtistMenuRow(artist: Artist) -> impl IntoView {
    let artist = RwSignal::new(artist);
    let releases_resource = Resource::new(move || artist.get().slug, get_releases);
    let releases = RwSignal::new(Vec::new());

    let artist_url = move || format!("/admin/artist/{}", artist.get().slug);
    let releases_url = move || format!("/admin/artist/{}/releases", artist.get().slug);
    let release_url = move || format!("/admin/artist/{}/releases/new", artist.get().slug);

    view! {
        <Transition fallback=move || {
            view! { "Loading..." }
        }>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    if let Ok(the_releases) = releases_resource.await {
                        releases.set(the_releases.releases);
                    }
                    view! {
                        <details>
                            <summary>
                                <a href=artist_url>{move || artist.get().name}</a>
                            </summary>
                            <ul>
                                <li>
                                    <a href=move || releases_url>"All Releases"</a>
                                </li>

                                <Show
                                    when=move || { !releases.get().is_empty() }
                                    fallback=move || {
                                        view! {
                                            <li>
                                                <a href=release_url>"No releases yet..."</a>
                                            </li>
                                        }
                                    }
                                >
                                    <For
                                        each=move || releases.get()
                                        key=|release| (release.slug.clone(), release.name.clone())
                                        children=move |release: Release| {
                                            view! {
                                                <li>
                                                    <ReleaseMenuRow release=release artist=artist />
                                                </li>
                                            }
                                        }
                                    />
                                </Show>
                                <li>
                                    <a href=release_url>"Create Release"</a>
                                </li>
                            </ul>
                        </details>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}

#[component]
fn ReleaseMenuRow(#[prop(into)] release: Release, artist: RwSignal<Artist>) -> impl IntoView {
    let release = RwSignal::new(release);
    let release_url = move || {
        format!(
            "/admin/artist/{}/release/{}",
            artist.get().slug,
            release.get().slug
        )
    };

    let primary_release_icon = move || {
        if release.get().primary_artist_id == artist.get().id {
            view! { <span title="Primary artist">"●"</span> }.into_any()
        } else {
            view! { <span title="Featured artist">"○"</span> }.into_any()
        }
    };
    view! { <a href=release_url>{primary_release_icon}" "{move || release.get().name}</a> }
}
