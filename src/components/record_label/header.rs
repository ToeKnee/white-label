use leptos::prelude::*;
use reactive_stores::Store;

use crate::components::utils::error::ErrorPage;
use crate::components::utils::loading::Loading;
use crate::models::record_label::RecordLabel;
use crate::routes::auth::get_user;
use crate::routes::auth::Login;
use crate::routes::auth::Logout;
use crate::routes::auth::Register;
use crate::routes::record_label::get_record_label;
use crate::store::GlobalState;
use crate::store::GlobalStateStoreFields;

/// Renders the record label page.
#[component]
pub fn LabelHeader() -> impl IntoView {
    let store = expect_context::<Store<GlobalState>>();
    let (record_label, set_record_label) = signal(store.record_label().get());
    let record_label_resource = Resource::new(move || record_label.get(), |_| get_record_label());

    let login = ServerAction::<Login>::new();
    let logout = ServerAction::<Logout>::new();
    let register = ServerAction::<Register>::new();

    let user = Resource::new(
        move || {
            (
                login.version().get(),
                register.version().get(),
                logout.version().get(),
            )
        },
        move |_| get_user(),
    );

    view! {
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
                        <div class="navbar bg-primary text-primary-content">
                            <div class="navbar-start">
                                <a href="/" class="text-xl btn btn-ghost">
                                    {record_label.name}
                                </a>
                            </div>

                            <div class="navbar-end">
                                <Transition fallback=move || {
                                    view! { <Loading /> }
                                }>
                                    {move || {
                                        user.get()
                                            .map(|user| match user {
                                                Err(e) => {
                                                    view! {
                                                        <a href="/register" class="btn btn-ghost">
                                                            "Register"
                                                        </a>

                                                        <a href="/login" class="btn btn-ghost">
                                                            "Login"
                                                        </a>
                                                        <span>{format!("Login error: {}", e)}</span>
                                                    }
                                                        .into_any()
                                                }
                                                Ok(None) => {
                                                    view! {
                                                        <a href="/register" class="btn btn-ghost">
                                                            "Register"
                                                        </a>

                                                        <a href="/login" class="btn btn-ghost">
                                                            "Login"
                                                        </a>
                                                    }
                                                        .into_any()
                                                }
                                                Ok(Some(user)) => {
                                                    view! {
                                                        <span>{user.username}</span>
                                                        <a href="/logout" class="btn btn-ghost">
                                                            "Log out"
                                                        </a>
                                                    }
                                                        .into_any()
                                                }
                                            })
                                    }}
                                </Transition>
                            </div>
                        </div>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}
