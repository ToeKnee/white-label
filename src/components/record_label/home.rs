//! This module contains the record label home page and its components.
use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::components::A;
use markdown;
use reactive_stores::Store;

use crate::components::utils::{error::ErrorPage, loading::Loading, status_badge::StatusBadge};
use crate::models::artist::Artist;
use crate::models::record_label::RecordLabel;
use crate::routes::record_label::{get_label_artists, get_record_label};
use crate::store::{GlobalState, GlobalStateStoreFields};
use crate::utils::shorten_string::shorten_string;

/// Renders the record label page.
#[component]
pub fn RecordLabelHome() -> impl IntoView {
    let store = expect_context::<Store<GlobalState>>();
    let record_label_resource = Resource::new_blocking(move || {}, |()| get_record_label());

    let artists = RwSignal::new(vec![]);
    let artists_resource =
        Resource::new_blocking(move || store.record_label().get().id, get_label_artists);

    view! {
        <Transition fallback=move || view! { <Loading /> }>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    match record_label_resource.await {
                        Ok(label) => {
                            let record_label = store.record_label();
                            *record_label.write() = label.record_label.clone();
                            label.record_label
                        }
                        Err(_) => RecordLabel::default(),
                    };
                    if store.record_label().get().id > 0 {
                        match artists_resource.await {
                            Ok(these_artists) => {
                                artists.set(these_artists.artists);
                            }
                            Err(x) => {
                                tracing::error!("Error while getting artists: {x:?}");
                            }
                        }
                    }

                    view! {
                        <Title text=store.record_label().get().name />
                        <article class="my-6 md:container md:mx-auto prose">
                            <h1>{store.record_label().get().name}</h1>
                            <div inner_html=markdown::to_html_with_options(
                                    &store.record_label().get().description,
                                    &markdown::Options::gfm(),
                                )
                                .unwrap_or_default() />

                            <h2>"Artists"</h2>
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
            <div class="w-96 shadow-xl not-prose card card-compact bg-base-100 bg-neutral text-neutral-content indicator">
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
