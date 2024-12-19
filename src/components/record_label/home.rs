use leptos::prelude::*;
use leptos_router::components::A;
use markdown;
use reactive_stores::Store;

use crate::models::artist::Artist;
use crate::models::record_label::RecordLabel;
use crate::routes::record_label::{get_label_artists, get_record_label};
use crate::state::GlobalState;
use crate::state::GlobalStateStoreFields;

/// Renders the record label page.
#[component]
pub fn RecordLabelHome() -> impl IntoView {
    let store = expect_context::<Store<GlobalState>>();
    let (record_label, set_record_label) = signal(store.record_label().get());
    let record_label_resource = Resource::new(move || record_label.get(), |_| get_record_label());
    view! {
        <Transition fallback=move || view! { <p>"Loading Record Label"</p> }>
            <ErrorBoundary fallback=|_| {
                view! { <p class="error-messages text-xs-center">"Something went wrong."</p> }
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
                        <h2>{record_label.name.clone()}</h2>
                        <div inner_html=markdown::to_html(&record_label.description) />

                        <h3 class="text-4xl font-bold">"Artists"</h3>
                        <ArtistList record_label />
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}

/// Render a list of artists for a record label.
#[component]
pub fn ArtistList(record_label: RecordLabel) -> impl IntoView {
    let store = expect_context::<Store<GlobalState>>();
    let (artists, set_artists) = signal(store.artists().get());

    let artists_resource = Resource::new(
        move || artists.get(),
        move |_artists| get_label_artists(record_label.clone()),
    );

    view! {
        <Transition fallback=move || view! { <p>"Loading Artists"</p> }>
            <ErrorBoundary fallback=|_| {
                view! { <p class="error-messages text-xs-center">"Something went wrong."</p> }
            }>
                {move || Suspend::new(async move {
                    if store.artists().get().is_empty() {
                        match artists_resource.await {
                            Ok(these_artists) => {
                                let artists = store.artists();
                                (*artists.write()).clone_from(&these_artists.artists);
                                (*set_artists.write()).clone_from(&these_artists.artists);
                                these_artists.artists
                            }
                            Err(_) => vec![Artist::default()],
                        };
                    }
                    let artists = store.artists().get();
                    let artist_rows = artists
                        .into_iter()
                        .map(|artist| {
                            view! {
                                <li>
                                    <ArtistRow artist />
                                </li>
                            }
                        })
                        .collect::<Vec<_>>();
                    view! { <ul>{artist_rows}</ul> }
                })}
            </ErrorBoundary>
        </Transition>
    }
}

#[component]
fn ArtistRow(#[prop(into)] artist: Artist) -> impl IntoView {
    view! {
        <div>
            <A href=format!("/artists/{}", artist.slug)>{artist.name}</A>
        </div>
    }
}
