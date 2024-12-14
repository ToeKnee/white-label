use leptos::prelude::*;
use leptos_router::components::A;
use markdown;

use crate::routes::label::get_label;

/// Renders the record label page.
//#[tracing::instrument]
#[component]
pub fn RecordLabelHome() -> impl IntoView {
    view! {
        <Transition fallback=move || view! { <p>"Loading Record Label"</p> }>
            <ErrorBoundary fallback=|_| {
                view! { <p class="error-messages text-xs-center">"Something went wrong."</p> }
            }>
                {move || Suspend::new(async move {
                    let resource = get_label().await;
                    let label = resource.unwrap().label;
                    view! {
                        <h2>{label.name}</h2>
                        <div inner_html=markdown::to_html(&label.description) />

                        <h3>"Artists"</h3>
                        <ul>
                            <li>
                                <A href=format!("/artist/janky-switch")>"Janky Switch"</A>
                            </li>
                        </ul>
                        <A href=format!("/artist/janky-switch")>"Janky Switch"</A>
                        <A href=format!("/artist/janky-switch")>"View Artists"</A>
                    }
                })}

            </ErrorBoundary>
        </Transition>
    }
}
