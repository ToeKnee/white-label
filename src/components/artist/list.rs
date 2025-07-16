//! This module contains the record label home page and its components.
use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::components::A;
use reactive_stores::Store;

use crate::components::utils::{error::ErrorPage, loading::Loading, status_badge::StatusBadge};
use crate::models::artist::Artist;
use crate::routes::record_label::get_label_artists;
use crate::store::{GlobalState, GlobalStateStoreFields};
use crate::utils::shorten_string::shorten_string;

/// Renders the record label page.
#[component]
pub fn ArtistsPage() -> impl IntoView {
    let store = expect_context::<Store<GlobalState>>();

    let artists = RwSignal::new(vec![]);
    let artists_resource = Resource::new(move || (), |()| get_label_artists());

    view! {
        <Transition fallback=move || view! { <Loading /> }>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    match artists_resource.await {
                        Ok(these_artists) => {
                            artists.set(these_artists.artists);
                        }
                        Err(x) => {
                            tracing::error!("Error while getting artists: {x:?}");
                        }
                    }

                    view! {
                        <Title text=store.record_label().get().name />
                        <article class="my-6 md:container md:mx-auto prose">
                            <h1>{store.record_label().get().name} " Artists"</h1>
                            <ArtistList artists=artists />
                        </article>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}

/// Render a list of artists for a record label.
#[component]
pub fn ArtistList(
    /// The artists to display
    artists: RwSignal<Vec<Artist>>,
) -> impl IntoView {
    view! {
        <div class="flex flex-wrap gap-4 justify-between">
            <Show when=move || { !artists.get().is_empty() }>
                <For
                    each=move || artists.get()
                    key=|artist| (artist.slug.clone(), artist.name.clone())
                    let(artist)
                >
                    <ArtistBox artist />
                </For>
            </Show>
        </div>
    }
}

#[component]
fn ArtistBox(
    /// The artist to display
    #[prop(into)]
    artist: Artist,
) -> impl IntoView {
    view! {
        <A href=format!("/artists/{}", artist.slug) attr:class="no-underline">
            <div class="w-96 shadow-xl not-prose card card-compact bg-neutral text-neutral-content indicator">
                <StatusBadge deleted_at=artist.deleted_at published_at=artist.published_at />
                <figure>
                    <img src=artist.primary_image_url() alt=artist.name.clone() />
                </figure>
                <div class="card-body">
                    <h2 class="card-title">{artist.name}</h2>

                    <p>{shorten_string(artist.description)}</p>
                </div>
            </div>
        </A>
    }
}
