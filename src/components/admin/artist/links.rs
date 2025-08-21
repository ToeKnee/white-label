//! Edit an artist music service and social links.

use leptos::prelude::*;
use leptos_meta::Title;
use reactive_stores::Store;

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
use crate::routes::links::{LinksResult, UpdateLinks, get_links};
use crate::store::{GlobalState, GlobalStateStoreFields};

/// Renders the edit artist links page.
#[component]
#[allow(clippy::too_many_lines)] // components are a pain to make smaller
pub fn EditArtistLinks() -> impl IntoView {
    Effect::new_isomorphic(move || {
        permission_or_redirect("label_owner", "/admin");
    });

    let store = expect_context::<Store<GlobalState>>();
    let artist = store.artist();

    let active_music_services = RwSignal::new(LinksResult::default().music_services);
    let active_social_media_services = RwSignal::new(LinksResult::default().social_media_services);
    let links_resource = Resource::new(move || artist.get().slug, get_links);

    let update_artist_links = ServerAction::<UpdateLinks>::new();
    let value = update_artist_links.value();
    let success = RwSignal::new(false);

    view! {
        <Title text=move || format!("{} Links", artist.get().name) />
        <h1>{move || view! { {artist.get().name} }}" Links"</h1>

        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    match links_resource.await {
                        Ok(services) => {
                            active_music_services.set(services.music_services);
                            active_social_media_services.set(services.social_media_services);
                        }
                        Err(e) => {
                            tracing::error!(
                                "Failed to fetch music services and social links. Error: {e:?}"
                            );
                        }
                    }
                    view! {
                        <ActionForm action=update_artist_links>
                            <div class="grid gap-6">
                                {move || {
                                    match value.get() {
                                        Some(Ok(services)) => {
                                            if active_music_services.get() != services.music_services
                                                || active_social_media_services.get()
                                                    != services.social_media_services
                                            {
                                                active_music_services.set(services.music_services);
                                                active_social_media_services
                                                    .set(services.social_media_services);
                                                success.set(true);
                                            }

                                            view! { "" }
                                                .into_any()
                                        }
                                        Some(Err(errors)) => {
                                            success.set(false);
                                            view! { <ServerErrors server_errors=Some(errors) /> }
                                                .into_any()
                                        }
                                        None => view! { "" }.into_any(),
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
                                    name="form[artist_slug]"
                                    value=move || artist.get().slug
                                /> <div class="divider">Music Services</div>
                                {move || {
                                    view! { <MusicServiceEdit active_music_services /> }
                                }} <div class="divider">Social Media</div>
                                {move || {
                                    view! {
                                        <SocialMediaServiceEdit active_social_media_services />
                                    }
                                }} <div class="flex flex-auto gap-6">
                                    <button class="flex-1 btn btn-primary">Update</button>
                                    {move || {
                                        if artist.get().deleted_at.is_some() {
                                            view! { <RestoreArtist artist=artist /> }.into_any()
                                        } else {
                                            view! { <DeleteArtist artist=artist /> }.into_any()
                                        }
                                    }}
                                </div>
                            </div>
                        </ActionForm>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}
