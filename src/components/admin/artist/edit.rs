use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;

use super::{
    delete::DeleteArtist,
    menu::{Menu, Page},
};
use crate::components::{
    admin::shared::{MarkdownField, PublishedAtField},
    files::upload::FileUploadWithProgress,
    utils::{
        error::ErrorPage, error::ServerErrors, loading::Loading,
        permissions::permission_or_redirect, success::Success,
    },
};
use crate::config::upload::UploadConfiguration;
use crate::models::artist::Artist;
use crate::routes::artist::{ArtistResult, UpdateArtist, get_artist};
use crate::utils::redirect::redirect;

/// Renders the create artist page.
#[component]
pub fn EditArtist() -> impl IntoView {
    Effect::new_isomorphic(move || {
        permission_or_redirect("label_owner", "/admin");
    });

    let params = use_params_map();
    let slug = RwSignal::new(String::new());
    Effect::new_isomorphic(move || {
        let s = params.read().get("slug").unwrap_or_default();
        slug.set(s);
    });

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
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    match artist_resource.await {
                        Ok(this_artist) => {
                            set_artist.set(this_artist.artist);
                        }
                        _ => {
                            redirect("/admin/artists");
                        }
                    };
                    view! {
                        <Title text=move || format!("{} Profile", artist.get().name) />
                        <h1>{move || view! { {artist.get().name} }}" Profile"</h1>

                        <Menu slug=slug selected=&Page::Profile />

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
                                }} <Form artist=artist slug=slug />

                            </div>
                        </ActionForm>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}

#[component]
fn Form(artist: ReadSignal<Artist>, slug: RwSignal<String>) -> impl IntoView {
    view! {
        <input type="text" class="hidden" name="artist_form[slug]" bind:value=slug />
        <div class="divider">Public</div>
        <label class="flex gap-2 items-center input input-bordered">
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

        <div class="divider">Images</div>
        {move || {
            view! {
                <FileUploadWithProgress config=UploadConfiguration::Artist slug=artist.get().slug />
            }
        }}
        <img src=move || artist.get().primary_image_url() alt=move || artist.get().name />

        <div class="divider">Private</div>
        {move || {
            view! {
                <PublishedAtField
                    field="artist_form[published_at]".to_string()
                    published_at=artist.get().published_at
                />
            }
        }}
        <div class="flex flex-auto gap-6">
            <button class="flex-1 btn btn-primary">Update</button>
            {move || {
                view! { <DeleteArtist artist=artist.get() /> }
            }}
        </div>
    }
}
