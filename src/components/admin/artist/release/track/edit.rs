//! Edit a track for a release in an artist's discography

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;
use reactive_stores::Store;

use super::{delete::DeleteTrack, restore::RestoreTrack};
use crate::components::{
    admin::shared::{
        artist_select::ArtistSelect, date_field::DateField, markdown_field::MarkdownField,
    },
    utils::{
        error::ErrorPage, error::ServerErrors, loading::Loading,
        permissions::permission_or_redirect, success::Success,
    },
};
use crate::models::{release::Release, track::Track};
use crate::routes::{
    release::get_release,
    track::{UpdateTrack, get_track},
};
use crate::store::{GlobalState, GlobalStateStoreFields};
use crate::utils::redirect::redirect;

/// Renders the edit track page.
#[component]
#[allow(clippy::too_many_lines)] // components are a pain to make smaller
pub fn EditTrack() -> impl IntoView {
    Effect::new_isomorphic(move || {
        permission_or_redirect("label_owner", "/admin");
    });

    let params = use_params_map();

    let store = expect_context::<Store<GlobalState>>();
    let artist = store.artist();
    let artist_ids = RwSignal::new(vec![]);

    let release = RwSignal::new(Release::default());
    let release_resource = Resource::new(
        move || {
            [
                artist.get().slug,
                params.read().get("release_slug").unwrap_or_default(),
            ]
        },
        |[artist_slug, release_slug]| get_release(artist_slug, release_slug),
    );

    let track_resource = Resource::new(
        move || {
            [
                artist.get().slug,
                params.read().get("release_slug").unwrap_or_default(),
                params.read().get("track_slug").unwrap_or_default(),
            ]
        },
        |[artist_slug, release_slug, track_slug]| get_track(artist_slug, release_slug, track_slug),
    );
    let track = RwSignal::new(Track::default());
    let update_track = ServerAction::<UpdateTrack>::new();
    let value = update_track.value();
    let success = RwSignal::new(false);

    let title = move || {
        format!(
            "{} - {} - {}",
            track.get().name,
            release.get().name,
            artist.get().name
        )
    };

    view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    match release_resource.await {
                        Ok(this_release) => {
                            release.set(this_release.release);
                        }
                        _ => {
                            redirect(&format!("/admin/artist/{}/releases", artist.get().slug));
                        }
                    }
                    match track_resource.await {
                        Ok(this_track) => {
                            track.set(this_track.track);
                            artist_ids.set(this_track.artists.iter().map(|a| a.id).collect());
                        }
                        _ => {
                            redirect(
                                &format!(
                                    "/admin/artist/{}/release/{}/tracks",
                                    artist.get().slug,
                                    params.read().get("release_slug").unwrap_or_default(),
                                ),
                            );
                        }
                    }

                    view! {
                        <Title text=title />
                        <h1>{title}</h1>

                        <ActionForm action=update_track>
                            <div class="grid gap-6">
                                {move || {
                                    match value.get() {
                                        Some(Ok(track_result)) => {
                                            let fresh_track = track_result.track;
                                            let fresh_artists = track_result.artists;
                                            if fresh_track.id > 0 {
                                                if fresh_track.slug != track.get().slug {
                                                    redirect(
                                                        &format!(
                                                            "/admin/artist/{}/release/{}/track/{}",
                                                            artist.get().slug,
                                                            params.read().get("release_slug").unwrap_or_default(),
                                                            fresh_track.slug,
                                                        ),
                                                    );
                                                }
                                                if fresh_track != track.get() {
                                                    track.set(fresh_track);
                                                    artist_ids
                                                        .set(fresh_artists.iter().map(|a| a.id).collect());
                                                }
                                                if !success.get() {
                                                    success.set(true);
                                                }
                                            } else {
                                                success.set(false);
                                            }

                                            view! { "" }
                                                .into_any()
                                        }
                                        Some(Err(errors)) => {
                                            view! { <ServerErrors server_errors=Some(errors) /> }
                                                .into_any()
                                        }
                                        None => view! { "" }.into_any(),
                                    }
                                }}
                                {move || {
                                    view! {
                                        <Success
                                            message=format!("{} Updated!", track.get().name)
                                            show=success.get()
                                        />
                                    }
                                }} <Form track artist_ids release />
                            </div>
                        </ActionForm>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}

#[component]
fn Form(
    track: RwSignal<Track>,
    artist_ids: RwSignal<Vec<i64>>,
    release: RwSignal<Release>,
) -> impl IntoView {
    view! {
        <input
            name="form[release_id]"
            type="number"
            class="hidden"
            value=move || release.get().id
        />
        <input type="text" class="hidden" name="form[slug]" value=move || { track.get().slug } />
        <label class="flex gap-2 items-center input">
            <input
                type="text"
                class="grow"
                placeholder="Track name"
                name="form[name]"
                value=move || track.get().name
            />
        </label>
        {move || {
            view! {
                <MarkdownField
                    title="Description".to_string()
                    field="form[description]".to_string()
                    markdown_text=track.get().description
                />
                <MarkdownField
                    title="Lyrics".to_string()
                    field="form[lyrics]".to_string()
                    markdown_text=track.get().lyrics
                />
            }
        }}

        {move || {
            view! {
                <ArtistSelect
                    primary_artist_id=release.get().primary_artist_id
                    artist_ids=artist_ids
                />
            }
        }}
        <label class="flex gap-2 items-center input">
            <input
                type="text"
                class="grow"
                placeholder="ISRC Code"
                name="form[isrc_code]"
                value=move || track.get().isrc_code
            />
        </label>
        <label class="flex gap-2 items-center input">
            <input
                type="number"
                min="0"
                max="999"
                class="grow"
                placeholder="BPM"
                name="form[bpm]"
                value=move || track.get().bpm
            />
        </label>
        <label class="flex gap-2 items-center input">
            <input
                type="number"
                min="0"
                max="999"
                class="grow"
                placeholder="Track Number"
                name="form[track_number]"
                value=move || track.get().track_number
            />
        </label>
        {move || {
            view! {
                <div class="flex gap-6">
                    <DateField
                        title="Published At".to_string()
                        field="form[published_at]"
                        date=track.get().published_at
                    />
                </div>
            }
        }}
        <div class="flex flex-auto gap-6">
            <button class="flex-1 btn btn-primary">Update</button>
            {move || {
                if track.get().deleted_at.is_some() {
                    view! { <RestoreTrack track=track /> }.into_any()
                } else {
                    view! { <DeleteTrack track=track /> }.into_any()
                }
            }}
        </div>
    }
}
