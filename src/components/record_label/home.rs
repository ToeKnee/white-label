use leptos::prelude::*;
use leptos_router::components::A;
use markdown;
use reactive_stores::Store;

use crate::models::artist::ArtistStoreFields;
#[cfg(feature = "ssr")]
use crate::routes::label::{get_label, get_label_artists};

use crate::state::GlobalState;
use crate::state::GlobalStateStoreFields;

/// Renders the record label page.
#[component]
pub fn RecordLabelHome() -> impl IntoView {
    view! {
        <Transition fallback=move || view! { <p>"Loading Record Label"</p> }>
            <ErrorBoundary fallback=|_| {
                view! { <p class="error-messages text-xs-center">"Something went wrong."</p> }
            }>
                {move || Suspend::new(async move {
                    let store = expect_context::<Store<GlobalState>>();
                    #[cfg(feature = "ssr")]
                    if store.record_label().get().name.is_empty() {
                        let record_label = store.record_label();
                        *record_label.write() = get_label().await.unwrap().label;
                    }
                    let record_label = store.record_label().get();

                    view! {
                        <h2>{record_label.name.clone()}</h2>
                        <div inner_html=markdown::to_html(&record_label.description) />

                        <h3 class="text-4xl font-bold">"Artists"</h3>
                        <ArtistList />
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}

/// Render a list of artists for a record label.
#[component]
pub fn ArtistList() -> impl IntoView {
    view! {
        <ul>
            <Transition fallback=move || view! { <p>"Loading Artists"</p> }>
                <ErrorBoundary fallback=|_| {
                    view! { <p class="error-messages text-xs-center">"Something went wrong."</p> }
                }>
                    {move || Suspend::new(async move {
                        let store = expect_context::<Store<GlobalState>>();
                        #[cfg(feature = "ssr")]
                        if store.record_label().get().name.is_empty() {
                            let record_label = store.record_label();
                            *record_label.write() = get_label().await.unwrap().label;
                        }
                        #[cfg(feature = "ssr")]
                        if store.artists().get().is_empty() {
                            let record_label = store.record_label().get();
                            let artists = store.artists();
                            *artists.write() = get_label_artists(record_label)
                                .await
                                .unwrap()
                                .artists;
                        }
                        let artists = store.artists();

                        view! {
                            <For each=move || artists key=|row| row.id().get() let:artist>
                                <li>
                                    <A href=format!("/artists/{}", artist.get().slug)>
                                        <h4 class="text-3xl font-bold">{artist.get().name}</h4>
                                    </A>

                                    <div inner_html=markdown::to_html(&artist.get().description) />
                                </li>
                            </For>
                        }
                    })}
                </ErrorBoundary>
            </Transition>
        </ul>
    }
}
