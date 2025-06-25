//! Header component for the application.
//! Displays the record label name and user menu.

use leptos::prelude::*;
use leptos_router::components::A;
use reactive_stores::Store;

use crate::app::UserContext;
use crate::components::utils::{error::ErrorPage, loading::Loading};
use crate::models::auth::User;
use crate::routes::record_label::get_record_label;
use crate::store::{GlobalState, GlobalStateStoreFields};

/// Renders the page header.
#[component]
pub fn LabelHeader() -> impl IntoView {
    let store = expect_context::<Store<GlobalState>>();
    let record_label_resource =
        Resource::new(move || store.record_label().get(), |_| get_record_label());

    view! {
        <div class="navbar bg-primary text-primary-content">
            <Transition fallback=Loading>
                <ErrorBoundary fallback=|_| {
                    ErrorPage
                }>
                    {move || Suspend::new(async move {
                        if store.record_label().get().id == 0 {
                            if let Ok(record_label_result) = record_label_resource.await {
                                store.record_label().set(record_label_result.record_label);
                            }
                        }
                        view! {
                            <div class="navbar-start">
                                <A href="/" attr:class="text-xl btn btn-ghost">
                                    {move || store.record_label().get().name}
                                </A>
                            </div>
                        }
                    })}
                </ErrorBoundary>
            </Transition>

            <div class="navbar-end">
                <UserMenu />
            </div>
        </div>
    }
}

/// Renders the user menu page.
#[component]
pub fn UserMenu() -> impl IntoView {
    let user_context = expect_context::<UserContext>();
    let (user, set_user) = signal(User::default());

    Effect::new(move || {
        set_user.set(user_context.0.get());
    });

    view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || {
                    if user.get().is_authenticated() {
                        view! {
                            <div class="flex-none">
                                <ul class="px-1 menu menu-horizontal">
                                    <li>
                                        <details>
                                            <summary>
                                                <div class="avatar">
                                                    <div class="w-10 rounded-full">
                                                        <img
                                                            alt=format!("{}'s Avatar", user.get().username)
                                                            src=user.get().avatar_url()
                                                        />
                                                    </div>
                                                </div>
                                                {user.get().username}
                                            </summary>
                                            <ul
                                                data-theme="light"
                                                class="p-2 rounded-t-none bg-base-100"
                                            >
                                                <li>
                                                    <A href="/profile" attr:class="btn btn-ghost">
                                                        "Profile"
                                                    </A>
                                                </li>
                                                <li>
                                                    <A
                                                        href="/profile/change-password"
                                                        attr:class="btn btn-ghost"
                                                    >
                                                        "Change Password"
                                                    </A>
                                                </li>
                                                {if user.get().permissions.contains("admin") {
                                                    view! {
                                                        <li>
                                                            <A href="/admin" attr:class="btn btn-ghost">
                                                                "Admin"
                                                            </A>
                                                        </li>
                                                    }
                                                        .into_any()
                                                } else {
                                                    view! { <li /> }.into_any()
                                                }}

                                                <li>
                                                    <A href="/logout" attr:class="btn btn-ghost">
                                                        "Log out"
                                                    </A>
                                                </li>
                                            </ul>
                                        </details>
                                    </li>
                                </ul>
                            </div>
                        }
                            .into_any()
                    } else {
                        view! {
                            <span>
                                <A href="/register" attr:class="btn btn-ghost">
                                    "Register"
                                </A>
                                <A href="/login" attr:class="btn btn-ghost">
                                    "Login"
                                </A>
                            </span>
                        }
                            .into_any()
                    }
                }}
            </ErrorBoundary>
        </Transition>
    }
}
