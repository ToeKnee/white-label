use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;

use crate::components::{
    admin::{
        artist::menu::{Menu, Page},
        shared::{artist_select::ArtistSelect, date_field::DateField, markdown_field::MarkdownField, release_select::ReleaseSelect},
    },
    utils::{error::ErrorPage, error::ServerErrors, loading::Loading, permissions::permission_or_redirect},
};
use crate::models::{artist::Artist, release::Release, track::Track};
use crate::routes::{
    artist::get_artist,
    release::get_release,
    track::{CreateTrack, TrackResult},
};
use crate::utils::redirect::redirect;

/// Renders the create track page.
#[component]
pub fn CreateTrack() -> impl IntoView {
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
    Effect::new_isomorphic(move || {
        artist_ids.set(vec![artist.get().id]);
    });

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
    Effect::new_isomorphic(move || {
        release_ids.set(vec![release.get().id]);
    });

    let track = RwSignal::new(Track::default());
    let create_track = ServerAction::<CreateTrack>::new();
    let value = Signal::derive(move || create_track.value().get().unwrap_or_else(|| Ok(TrackResult::default())));

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
                    view! {
                        <Title text="New Release" />
                        <h1>New Release</h1>

                        <Menu slug=artist_slug selected=&Page::Releases />

                        <ActionForm action=create_track>
                            <div class="grid gap-6">
                                {move || {
                                    match value.get() {
                                        Ok(track_result) => {
                                            let track = track_result.track;
                                            if track.id > 0 {
                                                redirect(
                                                    &format!(
                                                        "/admin/artist/{}/release/{}/track/{}",
                                                        artist_slug.get(),
                                                        release_slug.get(),
                                                        track.slug,
                                                    ),
                                                );
                                            }

                                            view! { "" }
                                                .into_any()
                                        }
                                        Err(errors) => {
                                            view! { <ServerErrors server_errors=Some(errors) /> }
                                                .into_any()
                                        }
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
        <label class="flex gap-2 items-center input">
            <input
                type="text"
                class="grow"
                placeholder="Track name"
                name="form[name]"
                value=move || track.get().name
            />
        </label>
        <MarkdownField
            title="Description".to_string()
            field="form[description]".to_string()
            markdown_text=String::new()
        />

        {move || {
            view! {
                <ArtistSelect primary_artist=artist.get() artist_ids=artist_ids />
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
        <button class="btn btn-primary">Create</button>
    }
}
