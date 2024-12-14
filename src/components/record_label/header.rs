use crate::routes::label::get_label;
use leptos::prelude::*;

/// Renders the record label page.
//#[tracing::instrument]
#[component]
pub fn LabelHeader() -> impl IntoView {
    view! {
        <Transition fallback=move || view! { <p>"Loading Record Label"</p> }>
            <ErrorBoundary fallback=|_| {
                view! { <p class="error-messages text-xs-center">"Something went wrong."</p> }
            }>
                {move || Suspend::new(async move {
                    let resource = get_label().await;
                    let label = resource.unwrap().label;
                    view! { <h1 class="text-4xl font-bold">{label.name}</h1> }
                })}

            </ErrorBoundary>
        </Transition>
    }
}
