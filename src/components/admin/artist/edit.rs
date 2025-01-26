use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;

use super::shared::{DescriptionFields, PublishedAtField};
use crate::components::admin::delete_artist::DeleteArtist;
use crate::components::utils::{
    error::ErrorPage, error::ServerErrors, loading::Loading, permissions::permission_or_redirect,
    success::Success,
};
use crate::models::artist::Artist;
use crate::routes::artist::{get_artist, ArtistResult, UpdateArtist};
use crate::utils::redirect::redirect;

/// Renders the create artist page.
#[component]
pub fn EditArtist() -> impl IntoView {
    Effect::new_isomorphic(move || {
        permission_or_redirect("label_owner", "/admin");
    });

    let params = use_params_map();
    let slug = params.read().get("slug").unwrap();
    let slug = RwSignal::new(slug);

    let (artist, set_artist) = signal(Artist::default());
    let artist_resource = Resource::new(move || slug, |slug| get_artist(slug.get()));

    let update_artist = ServerAction::<UpdateArtist>::new();
    let value = Signal::derive(move || {
        update_artist
            .value()
            .get()
            .unwrap_or_else(|| Ok(ArtistResult::default()))
    });
    let (success, set_success) = signal(false);

    view! {
        <Title text=move || format!("Edit {}", artist.get().name) />
        <h1>"Edit "{move || view! { {artist.get().name} }}</h1>

        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    if let Ok(this_artist) = artist_resource.await {
                        set_artist.set(this_artist.artist);
                    } else {
                        redirect("/admin/artists");
                    };
                    view! {
                        <ActionForm action=update_artist>
                            <div class="grid gap-6">
                                {move || {
                                    match value.get() {
                                        Ok(artist_result) => {
                                            let fresh_artist = artist_result.artist;
                                            if fresh_artist.id > 0 {
                                                if fresh_artist.slug != artist.get().slug {
                                                    set_artist.set(fresh_artist.clone());
                                                    slug.set(fresh_artist.clone().slug);
                                                    redirect(&format!("/admin/artist/{}", fresh_artist.slug));
                                                }
                                                if !success.get() {
                                                    set_artist.set(fresh_artist);
                                                    set_success.set(true);
                                                }
                                            } else {
                                                set_success.set(false);
                                            }

                                            view! { "" }
                                                .into_any()
                                        }
                                        Err(errors) => {
                                            set_success.set(false);
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
                                }}
                                <input
                                    type="text"
                                    class="hidden"
                                    name="artist_form[slug]"
                                    bind:value=slug
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
                                }} <div class="flex flex-auto gap-6">
                                    <button class="flex-1 btn btn-primary">Update</button>
                                    {move || {
                                        view! { <DeleteArtist artist=artist.get() /> }
                                    }}
                                </div>
                            </div>
                        </ActionForm>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    };
}
