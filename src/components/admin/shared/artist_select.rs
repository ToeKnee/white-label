//! Artist selection component for a form.

use leptos::prelude::*;

use crate::components::utils::{error::ErrorPage, loading::Loading};
use crate::models::artist::Artist;
use crate::routes::record_label::get_label_artists;

fn toggle_artist_id(artist_ids: RwSignal<Vec<i64>>, artist_id: i64) {
    let mut ids = artist_ids.get();
    if ids.contains(&artist_id) {
        ids.retain(|&id| id != artist_id);
    } else {
        ids.push(artist_id);
    }
    artist_ids.set(ids);
}

fn artist_ids_str(artist_ids: RwSignal<Vec<i64>>) -> String {
    artist_ids
        .get()
        .iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

/// Select component for choosing artists in a form.
#[component]
#[allow(clippy::needless_pass_by_value)]
pub fn ArtistSelect(
    /// The primary artist is the one that will be selected by default. It is the primary artist for the release.
    primary_artist: Artist,
    /// The list of artist IDs that are selected. This should also include the primary artist ID.
    artist_ids: RwSignal<Vec<i64>>,
) -> impl IntoView {
    let artists = RwSignal::new(vec![]);
    let artists_resource = Resource::new(move || primary_artist.label_id, get_label_artists);

    view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    if artists.get().is_empty() {
                        if let Ok(artist_list) = artists_resource.await {
                            artists.set(artist_list.artists);
                        }
                    }
                    view! {
                        <div class="flex flex-col gap-4">
                            <h2>"Artists"</h2>
                            <legend class="label">
                                <span class="label-text">"Main Artist"</span>
                            </legend>
                            <select class="select" name="form[primary_artist_id]">
                                {move || {
                                    let artist_rows = artists
                                        .get()
                                        .into_iter()
                                        .map(|artist| {
                                            let checked = artist.id == primary_artist.id;
                                            view! {
                                                <option class="option" value=artist.id selected=checked>
                                                    {artist.name}
                                                </option>
                                            }
                                        })
                                        .collect::<Vec<_>>();
                                    view! { {artist_rows} }
                                }}
                            </select>
                            {move || {
                                let artist_rows = artists
                                    .get()
                                    .into_iter()
                                    .map(|artist| {
                                        view! { <ArtistCheckbox artist artist_ids /> }
                                    })
                                    .collect::<Vec<_>>();
                                if artist_rows.is_empty() {
                                    view! { <p>"No artists found…"</p> }.into_any()
                                } else {
                                    view! {
                                        <fieldset class="flex flex-row flex-wrap gap-6 justify-center p-4 fieldset">
                                            <legend class="label">
                                                <span class="label-text">"All Artists"</span>
                                            </legend>
                                            {artist_rows}
                                        </fieldset>
                                    }
                                        .into_any()
                                }
                            }}
                        </div>
                    }
                })}
                <input
                    type="text"
                    class="hidden"
                    name="form[artist_ids]"
                    value=move || artist_ids_str(artist_ids)
                />
            </ErrorBoundary>
        </Transition>
    }
}

/// Checkbox component for selecting an artist in the form.
#[component]
pub fn ArtistCheckbox(
    /// The artist to display in the checkbox
    artist: Artist,
    /// The signal containing the list of selected artist IDs
    artist_ids: RwSignal<Vec<i64>>,
) -> impl IntoView {
    let checked = move || artist_ids.get().contains(&artist.id);
    view! {
        <label class="flex flex-row gap-4 label bg-base-100 border-base-300 rounded-box">
            <input
                class="checkbox"
                type="checkbox"
                checked=checked
                value=artist.id
                on:input:target=move |_ev| {
                    toggle_artist_id(artist_ids, artist.id);
                }
            />
            <div class="avatar not-prose">
                <div class="w-8 rounded-full">
                    <img src=artist.primary_image_url() alt=artist.name.clone() />
                </div>
            </div>
            {artist.name}
        </label>
    }
}
