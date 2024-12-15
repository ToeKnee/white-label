use leptos::prelude::*;
#[cfg(feature = "ssr")]
use leptos_router::hooks::use_params_map;
use markdown;
use reactive_stores::Store;

#[cfg(feature = "ssr")]
use crate::routes::artist::get_artist;
use crate::state::GlobalState;
use crate::state::GlobalStateStoreFields;

/// Renders the artist home page.
#[component]
pub fn ArtistPage() -> impl IntoView {
    view! {
        <Transition fallback=move || view! { <p>"Loading Artist"</p> }>
            <ErrorBoundary fallback=|_| {
                view! { <p class="error-messages text-xs-center">"Something went wrong."</p> }
            }>
                {move || Suspend::new(async move {
                    let store = expect_context::<Store<GlobalState>>();
                    #[cfg(feature = "ssr")]
                    if store.artist().get().name.is_empty() {
                        let params = use_params_map();
                        let artist = store.artist();
                        if let Some(s) = params.get().get("slug") {
                            *artist.write() = get_artist(s.to_string()).await.unwrap().artist;
                        } else {
                            *artist.write() = get_artist("janky-switch".to_string())
                                .await
                                .unwrap()
                                .artist;
                        }
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
