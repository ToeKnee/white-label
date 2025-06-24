//! Edit a track for a release in an artist's discography

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;

use super::delete::DeleteTrack;
use crate::components::{
    admin::shared::{
        artist_select::ArtistSelect, date_field::DateField, markdown_field::MarkdownField,
        release_select::ReleaseSelect,
    },
    utils::{
        error::ErrorPage, error::ServerErrors, loading::Loading,
        permissions::permission_or_redirect, success::Success,
    },
};
use crate::models::{artist::Artist, release::Release, track::Track};
use crate::routes::{
    artist::get_artist,
    release::get_release,
    track::{TrackResult, UpdateTrack, get_track},
};
use crate::utils::redirect::redirect;

/// Renders the edit track page.
#[component]
#[allow(clippy::too_many_lines)] // components are a pain to make smaller
pub fn EditTrack() -> impl IntoView {
    Effect::new_isomorphic(move || {
        permission_or_redirect("label_owner", "/admin");
    });

    let params = use_params_map();
    let artist_slug = RwSignal::new(String::new());
    Effect::new_isomorphic(move || {
        let s = params.read().get("slug").unwrap_or_default();
        artist_slug.set(s);
    });

    let artist = RwSignal::new(Artist::default());
    let artist_resource = Resource::new(move || artist_slug, |slug| get_artist(slug.get()));
    let artist_ids = RwSignal::new(vec![]);

    let release_slug = RwSignal::new(String::new());
    Effect::new_isomorphic(move || {
        let s = params.read().get("release_slug").unwrap_or_default();
        release_slug.set(s);
    });
    let release = RwSignal::new(Release::default());
    let release_resource = Resource::new(
        move || [artist_slug, release_slug],
        |[artist_slug, release_slug]| get_release(artist_slug.get(), release_slug.get()),
    );
    let release_ids = RwSignal::new(vec![]);

    let track_slug = RwSignal::new(String::new());
    Effect::new_isomorphic(move || {
        let s = params.read().get("track_slug").unwrap_or_default();
        track_slug.set(s);
    });
    let track_resource = Resource::new(
        move || [artist_slug, release_slug, track_slug],
        |[artist_slug, release_slug, track_slug]| {
            get_track(artist_slug.get(), release_slug.get(), track_slug.get())
        },
    );
    let track = RwSignal::new(Track::default());
    let update_track = ServerAction::<UpdateTrack>::new();
    let value = Signal::derive(move || {
        update_track
            .value()
            .get()
            .unwrap_or_else(|| Ok(TrackResult::default()))
    });
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
                        }
                        _ => {
                            redirect(&format!("/admin/artist/{}/releases", artist_slug.get()));
                        }
                    }
                    match track_resource.await {
                        Ok(this_track) => {
                            track.set(this_track.track);
                            artist_ids.set(this_track.artists.iter().map(|a| a.id).collect());
                            release_ids.set(this_track.releases.iter().map(|r| r.id).collect());
                        }
                        _ => {
                            redirect(
                                &format!(
                                    "/admin/artist/{}/release/{}/tracks",
                                    artist_slug.get(),
                                    release_slug.get(),
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
                                        Ok(track_result) => {
                                            let fresh_track = track_result.track;
                                            let fresh_artists = track_result.artists;
                                            let fresh_releases = track_result.releases;
                                            if fresh_track.id > 0 {
                                                if fresh_track.slug != track.get().slug {
                                                    redirect(
                                                        &format!(
                                                            "/admin/artist/{}/release/{}/track/{}",
                                                            artist_slug.get(),
                                                            release_slug.get(),
                                                            fresh_track.slug,
                                                        ),
                                                    );
                                                }
                                                if fresh_track != track.get() {
                                                    track.set(fresh_track);
                                                    artist_ids
                                                        .set(fresh_artists.iter().map(|a| a.id).collect());
                                                    release_ids
                                                        .set(fresh_releases.iter().map(|r| r.id).collect());
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
                                        Err(errors) => {
                                            view! { <ServerErrors server_errors=Some(errors) /> }
                                                .into_any()
                                        }
                                    }
                                }}
                                {move || {
                                    view! {
                                        <Success
                                            message=format!("{} Updated!", track.get().name)
                                            show=success.get()
                                        />
                                    }
                                }} <Form track artist artist_ids release release_ids />
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
    artist: RwSignal<Artist>,
    artist_ids: RwSignal<Vec<i64>>,
    release: RwSignal<Release>,
    release_ids: RwSignal<Vec<i64>>,
) -> impl IntoView {
    view! {
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
            }
        }}

        {move || {
            view! {
                <ArtistSelect
                    primary_artist_id=release.get().primary_artist_id
                    artist_ids=artist_ids
                    label_id=artist.get().label_id
                />
                <ReleaseSelect
                    artist_ids=artist_ids.get()
                    primary_release=release.get()
                    initial_release_ids=release_ids.get()
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
                view! { <DeleteTrack track=track.get() /> }
            }}
        </div>
    }
}
