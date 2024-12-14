use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use markdown;

use crate::routes::artist::get_artist;

/// Renders the artist home page.
//#[tracing::instrument]
#[component]
pub fn ArtistPage() -> impl IntoView {
    let params = use_params_map();

    let artist_res = Resource::new(
        move || params.get(),
        |slug| async move {
            if let Some(s) = slug.get("slug") {
                get_artist(s.to_string()).await
            } else {
                get_artist("janky-switch".to_string()).await
            }
        },
    );

    view! {
        <Transition fallback=move || view! { <p>"Loading Artist"</p> }>
            <ErrorBoundary fallback=|_| {
                view! { <p class="error-messages text-xs-center">"Something went wrong."</p> }
            }>
                {move || {
                    artist_res
                        .get()
                        .map(move |x| {
                            x.map(move |a| {
                                view! {
                                    <h1>{a.artist.name}</h1>
                                    <div inner_html=markdown::to_html(&a.artist.description) />
                                }
                            })
                        })
                }}
            </ErrorBoundary>
        </Transition>
    }
}
