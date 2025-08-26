//! Create a new track for a release in an artist's discography

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;
use reactive_stores::{Store, Subfield};

use crate::components::{
    admin::shared::{
        artist_select::ArtistSelect, date_field::DateField, markdown_field::MarkdownField,
    },
    utils::{
        error::ErrorPage, error::ServerErrors, loading::Loading,
        permissions::permission_or_redirect,
    },
};
use crate::store::{GlobalState, GlobalStateStoreFields};

use crate::models::{artist::Artist, release::Release, track::Track};
use crate::routes::{release::get_release, track::CreateTrack};
use crate::utils::redirect::redirect;

/// Renders the create track page.
#[component]
pub fn CreateTrack() -> impl IntoView {
    Effect::new_isomorphic(move || {
        permission_or_redirect("label_owner", "/admin");
    });

    let params = use_params_map();
    let store = expect_context::<Store<GlobalState>>();
    let artist = store.artist();

    let artist_ids = RwSignal::new(vec![]);
    Effect::new_isomorphic(move || {
        artist_ids.set(vec![artist.get().id]);
    });

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

    let track = RwSignal::new(Track::default());
    let create_track = ServerAction::<CreateTrack>::new();
    let value = create_track.value();

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

                    view! {
                        <Title text="New Release" />
                        <h1>New Track</h1>

                        <ActionForm action=create_track>
                            <div class="grid gap-6">
                                {move || {
                                    match value.get() {
                                        Some(Ok(track_result)) => {
                                            let track = track_result.track;
                                            if track.id > 0 {
                                                redirect(
                                                    &format!(
                                                        "/admin/artist/{}/release/{}/track/{}",
                                                        artist.get().slug,
                                                        params.read().get("release_slug").unwrap_or_default(),
                                                        track.slug,
                                                    ),
                                                );
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
                                }} <Form track artist artist_ids release />
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
    artist: Subfield<Store<GlobalState>, GlobalState, Artist>,
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
            view! { <ArtistSelect primary_artist_id=artist.get().id artist_ids=artist_ids /> }
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
        <button class="btn btn-primary">Create</button>
    }
}
