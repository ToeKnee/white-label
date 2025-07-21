//! Edit an artists images

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;

use crate::components::{
    files::upload::FileUploadWithProgress,
    utils::{
        error::ErrorPage, loading::Loading,
        permissions::permission_or_redirect,
    },
};
use crate::config::upload::UploadConfiguration;
use crate::models::artist::Artist;
use crate::routes::artist::get_artist;
use crate::utils::redirect::redirect;

/// Renders the edit artist image page.
#[component]
pub fn EditArtistImages() -> impl IntoView {
    Effect::new_isomorphic(move || {
        permission_or_redirect("label_owner", "/admin");
    });

    let params = use_params_map();

    let artist = RwSignal::new(Artist::default());
    let artist_resource = Resource::new(
        move || params.read().get("slug").unwrap_or_default(),
        get_artist,
    );

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
                        <Title text=move || format!("{} Images", artist.get().name) />
                        <h1>{move || view! { {artist.get().name} }}" Images"</h1>

                        <div class="grid gap-6">
                            <div role="tablist" class="tabs tabs-border not-prose">
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
                                    attr:class="tab"
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
                                    attr:class="tab tab-active"
                                >
                                    Images
                                </A>
                            </div>

                            <div class="divider">Images</div>
                            {move || {
                                view! {
                                    <FileUploadWithProgress
                                        config=UploadConfiguration::Artist
                                        slug=artist.get().slug
                                    />
                                }
                            }}
                            <img
                                src=move || artist.get().primary_image_url()
                                alt=move || artist.get().name
                            />
                        </div>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}
