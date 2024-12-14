use crate::routes::label::get_label;
use leptos::prelude::*;
use reactive_stores::Store;

use crate::state::GlobalState;
use crate::state::GlobalStateStoreFields;

/// Renders the record label page.
#[component]
pub fn LabelHeader() -> impl IntoView {
    let state = expect_context::<Store<GlobalState>>();
    let record_label = state.record_label();

    view! {
        <Transition fallback=move || view! { <p>"Loading Record Label"</p> }>
            <ErrorBoundary fallback=|_| {
                view! { <p class="error-messages text-xs-center">"Something went wrong."</p> }
            }>
                {move || Suspend::new(async move {
                    if record_label.get().name.is_empty() {
                        *record_label.write() = get_label().await.unwrap().label;
                    }
                    view! { <h1 class="text-4xl font-bold">{state.record_label().get().name}</h1> }
                })}

            </ErrorBoundary>
        </Transition>
    }
}
