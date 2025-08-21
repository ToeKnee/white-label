//! List tracks for an artist's release in the admin panel.

use leptos::prelude::*;
use leptos_router::{components::A, hooks::use_params_map};
use reactive_stores::Store;

use crate::components::utils::{
    error::ErrorPage, loading::Loading, permissions::permission_or_redirect,
};
use crate::models::{release::Release, track::Track};
use crate::routes::{release::get_release, track::get_tracks};
use crate::store::{GlobalState, GlobalStateStoreFields};
use crate::utils::redirect::redirect;

/// Renders the list tracks page.
#[component]
#[allow(clippy::too_many_lines)]
pub fn Tracks() -> impl IntoView {
    Effect::new_isomorphic(move || {
        permission_or_redirect("label_owner", "/admin");
    });

    let params = use_params_map();

    let store = expect_context::<Store<GlobalState>>();
    let artist = store.artist();

    let release_resource = Resource::new(
        move || {
            [
                artist.get().slug,
                params.read().get("release_slug").unwrap_or_default(),
            ]
        },
        |[artist_slug, release_slug]| get_release(artist_slug, release_slug),
    );
    let release = RwSignal::new(Release::default());

    let tracks_resource = Resource::new(
        move || {
            [
                artist.get().slug,
                params.read().get("release_slug").unwrap_or_default(),
            ]
        },
        |[artist_slug, release_slug]| get_tracks(artist_slug, release_slug),
    );
    let tracks = RwSignal::new(Vec::new());

    let title = RwSignal::new("Tracks".to_string());

    view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    match release_resource.await {
                        Ok(this_release) => {
                            release.set(this_release.release);
                            title.set(format!("{} Tracks", release.get().name));
                        }
                        _ => {
                            redirect(&format!("/admin/artist/{}", artist.get().slug));
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
                                    artist.get().slug,
                                    params.read().get("release_slug").unwrap_or_default(),
                                ),
                            );
                        }
                    }
                })} <h2>"Tracks"</h2> <div class="divider"></div> <div class="overflow-x-auto">
                    <table class="table">
                        <thead>
                            <tr>
                                <th>Name</th>
                                <th></th>
                            </tr>
                        </thead>
                        <tbody>
                            <Show
                                when=move || { !tracks.get().is_empty() }
                                fallback=|| {
                                    view! {
                                        <tr>
                                            <td colspan="2">No tracks found.</td>
                                        </tr>
                                    }
                                }
                            >
                                <For
                                    each=move || tracks.get()
                                    key=|track| (track.slug.clone(), track.name.clone())
                                    let(track)
                                >
                                    <TrackRow
                                        track=track
                                        artist_slug=artist.get().slug
                                        release_slug=params
                                            .read()
                                            .get("release_slug")
                                            .unwrap_or_default()
                                    />
                                </For>
                            </Show>
                            <tr>
                                <td></td>
                                <td>
                                    <A
                                        href=move || {
                                            format!(
                                                "/admin/artist/{}/release/{}/tracks/new",
                                                artist.get().slug,
                                                params.read().get("release_slug").unwrap_or_default(),
                                            )
                                        }
                                        attr:class="btn btn-primary"
                                    >
                                        Add
                                    </A>
                                </td>
                            </tr>
                        </tbody>
                        <tfoot>
                            <tr>
                                <th>Name</th>
                                <th></th>
                            </tr>
                        </tfoot>
                    </table>
                </div>

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
                <div class="flex gap-3 items-center">
                    <div class="avatar">
                        <div class="w-12 rounded-full not-prose">
                            <img class="m-0" src=track.primary_image_url() alt=track.name.clone() />
                        </div>
                    </div>
                    <div>
                        <div class="font-bold">
                            <A href=format!(
                                "/admin/artist/{}/release/{}/track/{}",
                                artist_slug,
                                release_slug,
                                track.slug,
                            )>{track.name.clone()}</A>
                        </div>
                        <div class="text-sm opacity-50">
                            {track.isrc_code.clone()} <br /> {track.slug.clone()}
                        </div>
                    </div>
                </div>
            </td>
            <td>
                <A
                    href=format!(
                        "/admin/artist/{}/release/{}/track/{}",
                        artist_slug,
                        release_slug,
                        track.slug,
                    )
                    attr:class="btn btn-primary"
                >
                    Edit
                </A>
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
