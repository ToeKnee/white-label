use leptos::prelude::*;
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
    let artist_slug = params
        .get()
        .get("slug")
        .unwrap_or_else(|| "janky-switch".to_string());

    let (artist, set_artist) = signal(Artist::default());
    let artist_resource = Resource::new(
        move || artist.get(),
        move |_| get_artist(artist_slug.clone()),
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
                        <article class="md:container md:mx-auto prose">
                            <h1>{artist.get().name}</h1>
                            <div inner_html=markdown::to_html(&artist.get().description) />
                        </article>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}
