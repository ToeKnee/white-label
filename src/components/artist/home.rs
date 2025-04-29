use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;
use markdown;

use crate::components::utils::error::ErrorPage;
use crate::components::utils::loading::Loading;
use crate::models::artist::Artist;
use crate::routes::artist::get_artist;

/// Renders the artist home page.
#[component]
pub fn ArtistPage() -> impl IntoView {
    let params = use_params_map();
    let slug = move || params.read().get("slug");

    let (artist, set_artist) = signal(Artist::default());
    let artist_resource = Resource::new(
        move || artist.get(),
        move |_| slug().map_or_else(|| get_artist(String::new()), get_artist),
    );
    view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    match artist_resource.await {
                        Ok(this_artist) => {
                            *set_artist.write() = this_artist.artist.clone();
                            this_artist.artist
                        }
                        Err(_) => Artist::default(),
                    };

                    view! {
                        <Title text=artist.get().name />
                        <article class="md:container md:mx-auto prose">
                            <h1>{artist.get().name}</h1>
                            <div inner_html=markdown::to_html_with_options(
                                    &artist.get().description,
                                    &markdown::Options::gfm(),
                                )
                                .unwrap_or_default() />
                            <img
                                src=move || artist.get().primary_image_url()
                                alt=move || artist.get().name
                                class="w-full h-auto"
                            />
                        </article>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}
