use leptos::prelude::*;
use reactive_stores::Store;

use crate::app::UserContext;
use crate::components::utils::error::ErrorPage;
use crate::components::utils::loading::Loading;
use crate::models::{auth::User, record_label::RecordLabel};
use crate::store::GlobalState;
use crate::store::GlobalStateStoreFields;
use crate::utils::shorten_string::shorten_string;

/// Renders the record label page.
#[component]
pub fn RecordLabel() -> impl IntoView {
    let store = expect_context::<Store<GlobalState>>();
    let (record_label, set_record_label) = signal(RecordLabel::default());

    let user_context = expect_context::<UserContext>();
    let (user, set_user) = signal(User::default());

    view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    set_user.set(user_context.0.get());
                    set_record_label.set(store.record_label().get());
                    view! {
                        {if user.get().permissions.contains("label_owner") {
                            view! {
                                <div class="basis-1/3">
                                    <div class="shadow-xl not-prose card bg-neutral text-neutral-content bg-base-100">
                                        <div class="card-body">
                                            <h2 class="card-title">{record_label.get().name}</h2>
                                            <p>"ISRC: "{record_label.get().isrc_base}" YY XXXXX"</p>
                                            <p>{shorten_string(record_label.get().description)}</p>
                                            <div class="justify-end card-actions">
                                                <a href="/admin/label" class="btn btn-primary">
                                                    Edit
                                                </a>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            }
                                .into_any()
                        } else {

                            view! { "" }
                                .into_any()
                        }}
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}
