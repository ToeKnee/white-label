use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use markdown;
use reactive_stores::Store;

use crate::models::artist::Artist;
use crate::routes::artist::get_artist;
use crate::state::GlobalState;
use crate::state::GlobalStateStoreFields;

/// Renders the artist home page.
#[component]
pub fn ArtistPage() -> impl IntoView {
    let params = use_params_map();
    let artist_slug = params
        .get()
        .get("slug")
        .unwrap_or_else(|| "janky-switch".to_string());

    let store = expect_context::<Store<GlobalState>>();
    let (artist, set_artist) = signal(store.artist().get());
    let artist_resource = Resource::new(
        move || artist.get(),
        move |_| get_artist(artist_slug.clone()),
    );
    view! {
        <Transition fallback=move || view! { <p>"Loading Artist"</p> }>
            <ErrorBoundary fallback=|_| {
                view! { <p class="error-messages text-xs-center">"Something went wrong."</p> }
            }>
                {move || Suspend::new(async move {
                    let artist_slug = params
                        .get()
                        .get("slug")
                        .unwrap_or_else(|| "janky-switch".to_string());
                    if store.artist().get().slug != artist_slug {
                        match artist_resource.await {
                            Ok(this_artist) => {
                                let artist = store.artist();
                                *artist.write() = this_artist.artist.clone();
                                *set_artist.write() = this_artist.artist.clone();
                                this_artist.artist
                            }
                            Err(_) => Artist::default(),
                        };
                    }
                    let artist = store.artist().get();

                    view! {
                        <h1>{artist.name}</h1>
                        <div inner_html=markdown::to_html(&artist.description) />
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}
