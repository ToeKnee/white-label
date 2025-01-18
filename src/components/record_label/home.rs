use leptos::prelude::*;
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
    let (record_label, set_record_label) = signal(store.record_label().get());
    let record_label_resource = Resource::new(move || record_label.get(), |_| get_record_label());
    view! {
        <Transition fallback=move || view! { <Loading /> }>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    if store.record_label().get().id == 0 {
                        match record_label_resource.await {
                            Ok(label) => {
                                let record_label = store.record_label();
                                *record_label.write() = label.label.clone();
                                *set_record_label.write() = label.label.clone();
                                label.label
                            }
                            Err(_) => RecordLabel::default(),
                        };
                    }
                    let record_label = store.record_label().get();

                    view! {
                        <article class="md:container md:mx-auto prose">
                            <h1>{record_label.name.clone()}</h1>
                            <div inner_html=markdown::to_html_with_options(
                                    &record_label.description,
                                    &markdown::Options::gfm(),
                                )
                                .unwrap() />

                            <h2>"Artists"</h2>
                            <ArtistList record_label />
                        </article>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}

/// Render a list of artists for a record label.
#[component]
pub fn ArtistList(record_label: RecordLabel) -> impl IntoView {
    let (artists, set_artists) = signal(vec![]);

    let artists_resource = Resource::new(
        move || artists.get(),
        move |_artists| get_label_artists(record_label.clone().id),
    );

    view! {
        <Transition fallback=move || view! { <Loading /> }>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    match artists_resource.await {
                        Ok(these_artists) => {
                            (*set_artists.write()).clone_from(&these_artists.artists);
                            these_artists.artists
                        }
                        Err(_) => vec![Artist::default()],
                    };
                    let artists = artists.get();
                    let artist_rows = artists
                        .into_iter()
                        .map(|artist| {

                            view! { <ArtistBox artist /> }
                        })
                        .collect::<Vec<_>>();
                    view! { <div class="grid grid-cols-3 grid-flow-row-dense">{artist_rows}</div> }
                })}
            </ErrorBoundary>
        </Transition>
    }
}

#[component]
fn ArtistBox(#[prop(into)] artist: Artist) -> impl IntoView {
    view! {
        <a href=format!("/artists/{}", artist.slug) class="no-underline">
            <div class="w-96 shadow-xl card card-compact bg-base-100 indicator">
                <StatusBadge deleted_at=artist.deleted_at published_at=artist.published_at />
                <figure>
                    <img
                        src="https://jankyswitch.com/images/Avatar240.webp"
                        alt=artist.name.clone()
                    />
                </figure>
                <div class="card-body">
                    <h2 class="card-title">{artist.name}</h2>

                    <p>{shorten_string(artist.description)}</p>
                </div>
            </div>
        </a>
    }
}
