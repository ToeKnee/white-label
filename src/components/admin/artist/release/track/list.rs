//! List tracks for an artist's release in the admin panel.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;

use crate::components::{
    admin::artist::menu::{Menu, Page},
    utils::{
        error::ErrorPage, loading::Loading, permissions::permission_or_redirect,
        status_badge::StatusBadge,
    },
};
use crate::models::{artist::Artist, release::Release, track::Track};
use crate::routes::{artist::get_artist, release::get_release, track::get_tracks};
use crate::utils::redirect::redirect;

/// Renders the list tracks page.
#[component]
#[allow(clippy::too_many_lines)]
pub fn Tracks() -> impl IntoView {
    Effect::new_isomorphic(move || {
        permission_or_redirect("label_owner", "/admin");
    });

    let params = use_params_map();
    let artist_slug = RwSignal::new(String::new());
    let release_slug = RwSignal::new(String::new());
    Effect::new_isomorphic(move || {
        artist_slug.set(params.read().get("slug").unwrap_or_default());
        release_slug.set(params.read().get("release_slug").unwrap_or_default());
    });

    let artist = RwSignal::new(Artist::default());
    let artist_resource = Resource::new(move || artist_slug, |slug| get_artist(slug.get()));

    let release_resource = Resource::new(
        move || [artist_slug, release_slug],
        |[artist_slug, release_slug]| get_release(artist_slug.get(), release_slug.get()),
    );
    let release = RwSignal::new(Release::default());

    let tracks_resource = Resource::new(
        move || [artist_slug, release_slug],
        |[artist_slug, release_slug]| get_tracks(artist_slug.get(), release_slug.get()),
    );
    let tracks = RwSignal::new(Vec::new());

    let title = RwSignal::new("Tracks".to_string());

    view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    match artist_resource.await {
                        Ok(this_artist) => {
                            artist.set(this_artist.artist);
                        }
                        _ => {
                            redirect("/admin/artists");
                        }
                    }
                    match release_resource.await {
                        Ok(this_release) => {
                            release.set(this_release.release);
                            title.set(format!("{} Tracks", release.get().name));
                        }
                        _ => {
                            redirect(&format!("/admin/artist/{}", artist_slug.get()));
                        }
                    }
                    match tracks_resource.await {
                        Ok(this_tracks) => {
                            tracks.set(this_tracks.tracks);
                        }
                        _ => {
                            redirect(
                                &format!(
                                    "/admin/artist/{}/release/{}",
                                    artist_slug.get(),
                                    release_slug.get(),
                                ),
                            );
                        }
                    }
                    let tracks_rows = tracks
                        .get()
                        .into_iter()
                        .map(|track| {
                            view! {
                                <TrackRow
                                    track=track
                                    artist_slug=artist_slug.get()
                                    release_slug=release_slug.get()
                                />
                            }
                        })
                        .collect::<Vec<_>>();
                    view! {
                        <Title text=title.get() />
                        <h1>{move || title.get()}</h1>

                        <Menu slug=artist_slug selected=&Page::Releases />

                        <div class="overflow-x-auto">
                            <table class="table">
                                <thead>
                                    <tr>
                                        <th></th>
                                        <th>Name</th>
                                        <th></th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {if tracks.get().is_empty() {
                                        view! {
                                            <tr>
                                                <td colspan="5">No tracks found.</td>
                                            </tr>
                                        }
                                            .into_any()
                                    } else {
                                        view! { {tracks_rows} }.into_any()
                                    }} <tr>
                                        <td colspan="2"></td>
                                        <td>
                                            <a
                                                href=format!(
                                                    "/admin/artist/{}/release/{}/tracks/new",
                                                    artist_slug.get(),
                                                    release_slug.get(),
                                                )
                                                class="btn btn-primary"
                                            >
                                                Add
                                            </a>
                                        </td>
                                    </tr>
                                </tbody>
                                <tfoot>
                                    <tr>
                                        <th></th>
                                        <th>Name</th>
                                        <th></th>
                                    </tr>
                                </tfoot>
                            </table>
                        </div>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}

#[component]
fn TrackRow(
    #[prop(into)] track: Track,
    artist_slug: String,
    release_slug: String,
) -> impl IntoView {
    view! {
        <tr>
            <td>
                <StatusBadge deleted_at=track.deleted_at published_at=track.published_at />
            </td>
            <td>
                <div class="flex gap-3 items-center">
                    <div class="avatar">
                        <div class="w-12 rounded-full not-prose">
                            <img class="m-0" src=track.primary_image_url() alt=track.name.clone() />
                        </div>
                    </div>
                    <div>
                        <div class="font-bold">
                            <a href=format!(
                                "/admin/artist/{}/release/{}/track/{}",
                                artist_slug,
                                release_slug,
                                track.slug,
                            )>{track.name.clone()}</a>
                        </div>
                        <div class="text-sm opacity-50">
                            {track.isrc_code.clone()} <br /> {track.slug.clone()}
                        </div>
                    </div>
                </div>
            </td>
            <td>
                <a
                    href=format!(
                        "/admin/artist/{}/release/{}/track/{}",
                        artist_slug,
                        release_slug,
                        track.slug,
                    )
                    class="btn btn-primary"
                >
                    Edit
                </a>
            </td>
        </tr>
    }
}

#[component]
fn ArtistRowFallback() -> impl IntoView {
    view! {
        <tr>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
        </tr>
        <tr>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
        </tr>
        <tr>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
        </tr>
    }
}
