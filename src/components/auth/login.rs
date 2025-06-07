//! Login page component.
use leptos::form::ActionForm;
use leptos::prelude::*;

use crate::app::UserContext;
use crate::components::utils::{error::ErrorPage, error::ServerErrors};
use crate::models::auth::User;
use crate::routes::auth::Login;
use crate::utils::redirect::redirect;

/// Renders the login page.
#[component]
pub fn Login() -> impl IntoView {
    let login = ServerAction::<Login>::new();
    let value = Signal::derive(move || login.value().get().unwrap_or_else(|| Ok(User::default())));

    let user_context = expect_context::<UserContext>();

    view! {
        <article class="md:container md:mx-auto prose">
            <h1>Login</h1>

            <ActionForm action=login>
                <div class="grid gap-6">
                    <ErrorBoundary fallback=|_| {
                        ErrorPage
                    }>
                        {move || {
                            match value.get() {
                                Ok(user) => {
                                    user_context.1.set(user.clone());
                                    if user.is_authenticated() {
                                        redirect("/");
                                    }
                                    view! { "" }.into_any()
                                }
                                Err(errors) => {
                                    view! { <ServerErrors server_errors=Some(errors) /> }.into_any()
                                }
                            }
                        }}
                    </ErrorBoundary>

                    <fieldset class="flex flex-col gap-6 fieldset">
                        <label class="flex gap-2 items-center input">
                            <svg
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 16 16"
                                fill="currentColor"
                                class="w-4 h-4 opacity-70"
                            >
                                <path d="M8 8a3 3 0 1 0 0-6 3 3 0 0 0 0 6ZM12.735 14c.618 0 1.093-.561.872-1.139a6.002 6.002 0 0 0-11.215 0c-.22.578.254 1.139.872 1.139h9.47Z" />
                            </svg>
                            <input
                                type="text"
                                class="grow"
                                placeholder="Username"
                                name="username"
                            />
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
                            <input
                                type="password"
                                class="grow"
                                placeholder="Password"
                                name="password"
                            />
                        </label>

                        <label class="label" for="remember">
                            <input
                                type="checkbox"
                                checked="checked"
                                class="checkbox"
                                name="remember"
                                id="remember"
                            />
                            Remember me
                        </label>

                        <button class="btn btn-primary">Login</button>
                    </fieldset>
                </div>
            </ActionForm>
        </article>
    }
}
