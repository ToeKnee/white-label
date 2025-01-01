use leptos::prelude::*;
use reactive_stores::Store;

use crate::app::UserContext;
use crate::components::utils::error::ErrorPage;
use crate::components::utils::loading::Loading;
use crate::models::auth::User;
use crate::models::record_label::RecordLabel;
use crate::routes::record_label::get_record_label;
use crate::store::GlobalState;
use crate::store::GlobalStateStoreFields;

/// Renders the record label page.
#[component]
pub fn LabelHeader() -> impl IntoView {
    let store = expect_context::<Store<GlobalState>>();
    let (record_label, set_record_label) = signal(store.record_label().get());
    let record_label_resource = Resource::new(move || record_label.get(), |_| get_record_label());

    let user_context = expect_context::<UserContext>();
    let (user, set_user) = signal(User::default());

    view! {
        <div class="navbar bg-primary text-primary-content">
            <Transition fallback=move || view! { <Loading /> }>
                <ErrorBoundary fallback=|_| {
                    ErrorPage
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
                            <div class="navbar-start">
                                <a href="/" class="text-xl btn btn-ghost">
                                    {record_label.name}
                                </a>
                            </div>
                        }
                    })}
                </ErrorBoundary>
            </Transition>
            <Transition fallback=move || view! { <Loading /> }>
                <ErrorBoundary fallback=|_| {
                    ErrorPage
                }>
                    {move || Suspend::new(async move {
                        set_user.set(user_context.0.get().clone());
                        view! {
                            <div class="navbar-end">
                                {if user.get().is_authenticated() {
                                    view! {
                                        <span>
                                            <span>{user.get().username}</span>
                                            <a href="/logout" class="btn btn-ghost">
                                                "Log out"
                                            </a>
                                        </span>
                                    }
                                        .into_any()
                                } else {
                                    view! {
                                        <span>
                                            <a href="/register" class="btn btn-ghost">
                                                "Register"
                                            </a>
                                            <a href="/login" class="btn btn-ghost">
                                                "Login"
                                            </a>
                                        </span>
                                    }
                                        .into_any()
                                }}
                            </div>
                        }
                    })}
                </ErrorBoundary>
            </Transition>
        </div>
    }
}
