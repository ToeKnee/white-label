use leptos::prelude::*;
use reactive_stores::Store;

use super::shared::{DescriptionFields, PublishedAtField};
use crate::components::utils::{
    error::ErrorPage, error::ServerErrors, loading::Loading, permissions::permission_or_redirect,
};
use crate::models::artist::Artist;
use crate::routes::artist::{ArtistResult, CreateArtist};
use crate::store::{GlobalState, GlobalStateStoreFields};
use crate::utils::redirect::redirect;

/// Renders the create artist page.
#[component]
pub fn CreateArtist() -> impl IntoView {
    Effect::new_isomorphic(move || {
        permission_or_redirect("label_owner", "/admin");
    });

    let store = expect_context::<Store<GlobalState>>();
    let (record_label, _set_record_label) = signal(store.record_label().get());

    let (artist, set_artist) = signal(Artist::default());

    // Set the record label id to the artist
    Effect::new_isomorphic(move || {
        let mut a = artist.get();
        if a.label_id == 0 && record_label.get().id > 0 {
            a.label_id = record_label.get().id;
            set_artist.set(a);
        }
    });

    let create_artist = ServerAction::<CreateArtist>::new();
    let value = Signal::derive(move || {
        create_artist
            .value()
            .get()
            .unwrap_or_else(|| Ok(ArtistResult::default()))
    });

    let var_name = view! {
        <h1>Create Artist</h1>

        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    view! {
                        <ActionForm action=create_artist>
                            <div class="grid gap-6">
                                {move || {
                                    match value.get() {
                                        Ok(artist_result) => {
                                            let artist = artist_result.artist;
                                            if artist.id > 0 {
                                                redirect(&format!("/admin/artist/{}", artist.slug));
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
                                <input
                                    type="text"
                                    class="hidden"
                                    placeholder=""
                                    name="artist_form[label_id]"
                                    value=record_label.get().id
                                /> <div class="divider">Public</div>
                                <label class="flex gap-2 items-center input input-bordered">
                                    <input
                                        type="text"
                                        class="grow"
                                        placeholder="Artist name"
                                        name="artist_form[name]"
                                        value=artist.get().name
                                    />
                                </label> <DescriptionFields artist=artist.get() />
                                <div class="divider">Private</div>
                                {move || {
                                    view! {
                                        <PublishedAtField published_at=artist.get().published_at />
                                    }
                                }} <button class="btn btn-primary">Create</button>
                            </div>
                        </ActionForm>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    };
    var_name
}
