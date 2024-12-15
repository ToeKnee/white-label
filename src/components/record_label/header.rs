#[cfg(feature = "ssr")]
use crate::routes::label::get_label;
use leptos::prelude::*;
use reactive_stores::Store;

use crate::state::GlobalState;
use crate::state::GlobalStateStoreFields;

/// Renders the record label page.
#[component]
pub fn LabelHeader() -> impl IntoView {
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
                    view! { <h1 class="text-4xl font-bold">{record_label.name}</h1> }
                })}

            </ErrorBoundary>
        </Transition>
    }
}
