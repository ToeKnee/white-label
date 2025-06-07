//! Change Password Component
use leptos::form::ActionForm;
use leptos::prelude::*;
use leptos_meta::Title;

use crate::components::utils::{
    error::ErrorPage, error::ServerErrors, loading::Loading,
    permissions::authenticated_or_redirect, success::Success,
};
use crate::models::auth::User;
use crate::routes::auth::ChangePassword;

/// ChangePassword component allows users to change their password.
#[component]
pub fn ChangePassword() -> impl IntoView {
    Effect::new_isomorphic(move || {
        authenticated_or_redirect("/login");
    });

    let change_password = ServerAction::<ChangePassword>::new();
    let value =
        Signal::derive(move || change_password.value().get().unwrap_or(Ok(User::default())));
    let (success, set_success) = signal(false);

    view! {
        <article class="md:container md:mx-auto prose">
            <Title text="Change Password" />
            <h1>{"Change Password"}</h1>
            <Transition fallback=Loading>
                <ErrorBoundary fallback=|_| {
                    ErrorPage
                }>
                    {move || Suspend::new(async move {
                        view! {
                            <ActionForm action=change_password>
                                <div class="grid gap-6">
                                    {move || {
                                        match value.get() {
                                            Ok(user) => {
                                                if user.is_authenticated() && !success.get() {
                                                    set_success.set(true);
                                                }
                                                view! { "" }.into_any()
                                            }
                                            Err(errors) => {
                                                set_success.set(false);
                                                view! { <ServerErrors server_errors=Some(errors) /> }
                                                    .into_any()
                                            }
                                        }
                                    }}
                                    {move || {
                                        view! {
                                            <Success
                                                message="Password Updated!".to_string()
                                                show=success.get()
                                            />
                                        }
                                    }} <Form />
                                </div>
                            </ActionForm>
                        }
                    })}
                </ErrorBoundary>
            </Transition>
        </article>
    }
}

/// Form component for changing the password.
#[component]
fn Form() -> impl IntoView {
    view! {
        <label class="flex gap-2 items-center input">
            <svg
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 16 16"
                fill="currentColor"
                class="w-4 h-4 opacity-70"
            >
                <path
                    fill-rule="evenodd"
                    d="M14 6a4 4 0 0 1-4.899 3.899l-1.955 1.955a.5.5 0 0 1-.353.146H5v1.5a.5.5 0 0 1-.5.5h-2a.5.5 0 0 1-.5-.5v-2.293a.5.5 0 0 1 .146-.353l3.955-3.955A4 4 0 1 1 14 6Zm-4-2a.75.75 0 0 0 0 1.5.5.5 0 0 1 .5.5.75.75 0 0 0 1.5 0 2 2 0 0 0-2-2Z"
                    clip-rule="evenodd"
                />
            </svg>
            <input type="password" class="grow" name="password_form[password]" />
        </label>
        <label class="flex gap-2 items-center input">
            <svg
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 16 16"
                fill="currentColor"
                class="w-4 h-4 opacity-70"
            >
                <path
                    fill-rule="evenodd"
                    d="M14 6a4 4 0 0 1-4.899 3.899l-1.955 1.955a.5.5 0 0 1-.353.146H5v1.5a.5.5 0 0 1-.5.5h-2a.5.5 0 0 1-.5-.5v-2.293a.5.5 0 0 1 .146-.353l3.955-3.955A4 4 0 1 1 14 6Zm-4-2a.75.75 0 0 0 0 1.5.5.5 0 0 1 .5.5.75.75 0 0 0 1.5 0 2 2 0 0 0-2-2Z"
                    clip-rule="evenodd"
                />
            </svg>
            <input type="password" class="grow" name="password_form[new_password]" />
        </label>
        <label class="flex gap-2 items-center input">
            <svg
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 16 16"
                fill="currentColor"
                class="w-4 h-4 opacity-70"
            >
                <path
                    fill-rule="evenodd"
                    d="M14 6a4 4 0 0 1-4.899 3.899l-1.955 1.955a.5.5 0 0 1-.353.146H5v1.5a.5.5 0 0 1-.5.5h-2a.5.5 0 0 1-.5-.5v-2.293a.5.5 0 0 1 .146-.353l3.955-3.955A4 4 0 1 1 14 6Zm-4-2a.75.75 0 0 0 0 1.5.5.5 0 0 1 .5.5.75.75 0 0 0 1.5 0 2 2 0 0 0-2-2Z"
                    clip-rule="evenodd"
                />
            </svg>
            <input type="password" class="grow" name="password_form[new_password_confirmation]" />
        </label>
        <div class="flex flex-auto gap-6">
            <button class="flex-1 btn btn-primary">"Change Password"</button>
        </div>
    }
}
