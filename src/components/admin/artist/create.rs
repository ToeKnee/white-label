//! Create an artist

use leptos::prelude::*;
use leptos_meta::Title;
use reactive_stores::Store;

use crate::components::{
    admin::shared::{date_field::DateField, markdown_field::MarkdownField},
    utils::{
        error::ErrorPage, error::ServerErrors, loading::Loading,
        permissions::permission_or_redirect,
    },
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
    let record_label = move || store.record_label().get();

    let (artist, set_artist) = signal(Artist::default());

    // Set the record label id to the artist
    Effect::new_isomorphic(move || {
        let mut a = artist.get();
        if a.label_id == 0 && record_label().id > 0 {
            a.label_id = record_label().id;
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

    view! {
        <Title text="Create Artist" />
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
                                            tracing::info!("Artist created: {:?}", artist_result);
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
                                    value=move || artist.get().label_id
                                /> <div class="divider">Public</div>
                                <label class="flex gap-2 items-center input">
                                    <input
                                        type="text"
                                        class="grow"
                                        placeholder="Artist name"
                                        name="artist_form[name]"
                                    />
                                </label>
                                {move || {
                                    view! {
                                        <MarkdownField
                                            title="Description".to_string()
                                            field="artist_form[description]".to_string()
                                            markdown_text=artist.get().description
                                        />
                                    }
                                }} <label class="flex gap-2 items-center input">
                                    <input
                                        type="text"
                                        class="grow"
                                        placeholder="Website"
                                        name="artist_form[website]"
                                        value=move || artist.get().website
                                    />
                                </label> <div class="divider">Private</div>
                                {move || {
                                    view! {
                                        <DateField
                                            title="Published at".to_string()
                                            field="artist_form[published_at]"
                                            date=artist.get().published_at
                                        />
                                    }
                                }} <button class="btn btn-primary">Create</button>
                            </div>
                        </ActionForm>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}
