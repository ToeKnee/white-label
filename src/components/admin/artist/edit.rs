//! Edit an artist

use leptos::prelude::*;
use leptos_meta::Title;
use reactive_stores::Store;

use super::{delete::DeleteArtist, restore::RestoreArtist};
use crate::components::{
    admin::shared::{date_field::DateField, markdown_field::MarkdownField},
    utils::{
        error::ErrorPage, error::ServerErrors, loading::Loading,
        permissions::permission_or_redirect, success::Success,
    },
};
use crate::models::artist::Artist;
use crate::routes::artist::{ArtistResult, UpdateArtist};
use crate::store::{GlobalState, GlobalStateStoreFields};
use crate::utils::redirect::redirect;

/// Renders the edit artist page.
#[component]
#[allow(clippy::too_many_lines)] // components are a pain to make smaller
pub fn EditArtist() -> impl IntoView {
    Effect::new_isomorphic(move || {
        permission_or_redirect("label_owner", "/admin");
    });

    let store = expect_context::<Store<GlobalState>>();
    let artist = RwSignal::new(
        store
            .artist()
            .get_untracked()
            .unwrap_or_else(Artist::default),
    );
    Effect::new(move || {
        artist.set(store.artist().get().unwrap_or_else(Artist::default));
    });
    let update_artist = ServerAction::<UpdateArtist>::new();
    let value = Signal::derive(move || {
        update_artist
            .value()
            .get()
            .unwrap_or_else(|| Ok(ArtistResult::default()))
    });
    let success = RwSignal::new(false);

    view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    view! {
                        <Title text=move || format!("{} Profile", artist.get().name) />
                        <h1>{move || view! { {artist.get().name} }}" Profile"</h1>

                        <ActionForm action=update_artist>
                            <div class="grid gap-6">
                                {move || {
                                    match value.get() {
                                        Ok(artist_result) => {
                                            let fresh_artist = artist_result.artist;
                                            if fresh_artist.id > 0 {
                                                store.artist().set(Some(fresh_artist.clone()));
                                                if fresh_artist.slug != artist.get().slug {
                                                    redirect(&format!("/admin/artist/{}", fresh_artist.slug));
                                                }
                                                artist.set(fresh_artist);
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
                                            success.set(false);
                                            view! { <ServerErrors server_errors=Some(errors) /> }
                                                .into_any()
                                        }
                                    }
                                }}
                                {move || {
                                    view! {
                                        <Success
                                            message=format!("{} Updated!", artist.get().name)
                                            show=success.get()
                                        />
                                    }
                                }} <Form artist=artist />
                            </div>
                        </ActionForm>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}

#[component]
fn Form(artist: RwSignal<Artist>) -> impl IntoView {
    view! {
        <input
            type="text"
            class="hidden"
            name="artist_form[slug]"
            value=move || artist.get().slug
        />
        <div class="divider">Public</div>
        <label class="flex gap-2 items-center input">
            <input
                type="text"
                class="grow"
                placeholder="Artist name"
                name="artist_form[name]"
                value=move || artist.get().name
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
        }}
        <label class="flex gap-2 items-center input">
            <input
                type="text"
                class="grow"
                placeholder="Website"
                name="artist_form[website]"
                value=move || artist.get().website
            />
        </label>

        <div class="divider">Private</div>
        {move || {
            view! {
                <DateField
                    title="Published at".to_string()
                    field="artist_form[published_at]"
                    date=artist.get().published_at
                />
            }
        }}
        <div class="flex flex-auto gap-6">
            <button class="flex-1 btn btn-primary">Update</button>
            {move || {
                if artist.get().deleted_at.is_some() {
                    view! { <RestoreArtist artist=artist /> }.into_any()
                } else {
                    view! { <DeleteArtist artist=artist /> }.into_any()
                }
            }}
        </div>
    }
}
