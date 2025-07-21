//! Edit an artist music service and social links.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;

use super::{delete::DeleteArtist, restore::RestoreArtist};
use crate::components::{
    admin::artist::{music_service::MusicServiceEdit, social_media::SocialMediaServiceEdit},
    utils::{
        error::{ErrorPage, ServerErrors},
        loading::Loading,
        permissions::permission_or_redirect,
        success::Success,
    },
};
use crate::models::artist::Artist;
use crate::routes::artist::{ArtistResult, UpdateArtist, get_artist};
use crate::utils::redirect::redirect;

/// Renders the edit artist links page.
#[component]
#[allow(clippy::too_many_lines)] // components are a pain to make smaller
pub fn EditArtistLinks() -> impl IntoView {
    Effect::new_isomorphic(move || {
        permission_or_redirect("label_owner", "/admin");
    });

    let params = use_params_map();

    let artist = RwSignal::new(Artist::default());
    let artist_resource = Resource::new(
        move || params.read().get("slug").unwrap_or_default(),
        get_artist,
    );
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
                    if !params.read().get("slug").unwrap_or_default().is_empty() {
                        match artist_resource.await {
                            Ok(this_artist) => {
                                if this_artist.artist.id > 0 {
                                    artist.set(this_artist.artist);
                                } else {
                                    tracing::error!("Artist not found: {:?}", this_artist);
                                }
                            }
                            _ => {
                                redirect("/admin/artists");
                            }
                        }
                    }

                    view! {
                        <Title text=move || format!("{} Links", artist.get().name) />
                        <h1>{move || view! { {artist.get().name} }}" Links"</h1>

                        <ActionForm action=update_artist>
                            <div class="grid gap-6">
                                {move || {
                                    match value.get() {
                                        Ok(artist_result) => {
                                            let fresh_artist = artist_result.artist;
                                            if fresh_artist.id > 0 {
                                                if fresh_artist.slug != artist.get().slug {
                                                    redirect(&format!("/admin/artist/{}", fresh_artist.slug));
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
                                }} <div role="tablist" class="tabs tabs-border not-prose">
                                    <A
                                        href=move || {
                                            format!(
                                                "/admin/artist/{}",
                                                params.read().get("slug").unwrap_or_default(),
                                            )
                                        }
                                        attr:role="tab"
                                        attr:class="tab"
                                    >
                                        Information
                                    </A>
                                    <A
                                        href=move || {
                                            format!(
                                                "/admin/artist/{}/links",
                                                params.read().get("slug").unwrap_or_default(),
                                            )
                                        }
                                        attr:role="tab"
                                        attr:class="tab tab-active"
                                    >
                                        Links
                                    </A>
                                    <A
                                        href=move || {
                                            format!(
                                                "/admin/artist/{}/images",
                                                params.read().get("slug").unwrap_or_default(),
                                            )
                                        }
                                        attr:role="tab"
                                        attr:class="tab"
                                    >
                                        Images
                                    </A>
                                </div> <Form artist=artist />
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
    let params = use_params_map();

    view! {
        <input
            type="text"
            class="hidden"
            name="artist_form[slug]"
            value=move || artist.get().slug
        />

        <div class="divider">Music Services</div>
        {move || {
            view! { <MusicServiceEdit artist_slug=params.read().get("slug").unwrap_or_default() /> }
        }}

        <div class="divider">Social Media</div>
        {move || {
            view! {
                <SocialMediaServiceEdit artist_slug=params.read().get("slug").unwrap_or_default() />
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
