use leptos::prelude::*;
use leptos_router::components::A;
use reactive_stores::Store;

use crate::models::record_label::RecordLabel;
use crate::routes::record_label::get_record_label;
use crate::state::GlobalState;
use crate::state::GlobalStateStoreFields;

/// Renders the record label page.
#[component]
pub fn LabelHeader() -> impl IntoView {
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
                                let store_record_label = store.record_label();
                                *store_record_label.write() = label.label.clone();
                                *set_record_label.write() = label.label.clone();
                                label.label
                            }
                            Err(_) => RecordLabel::default(),
                        };
                    }
                    let record_label = store.record_label().get();

                    view! {
                        <h1>
                            <A href="/">{record_label.name.clone()}</A>
                        </h1>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}
