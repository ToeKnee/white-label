use leptos::form::ActionForm;
use leptos::prelude::*;
use leptos_router::{hooks::use_navigate, NavigateOptions};

use crate::app::UserContext;
use crate::components::utils::error::ServerErrors;
use crate::models::auth::User;
use crate::routes::auth::Register;

/// Renders the register page.
#[component]
pub fn Register() -> impl IntoView {
    let register = ServerAction::<Register>::new();
    let value = Signal::derive(move || {
        register
            .value()
            .get()
            .unwrap_or_else(|| Ok(User::default()))
    });
    let (server_errors, set_server_errors) = signal(Option::<ServerFnError>::None);

    let user_context = expect_context::<UserContext>();

    view! {
        <article class="md:container md:mx-auto prose">
            <h1>Register</h1>
            <ActionForm action=register>
                <div class="grid gap-6">
                    <Suspense>
                        {move || {
                            match value.get() {
                                Ok(user_result) => {
                                    let this_user = user_result;
                                    user_context.1.set(this_user.clone());
                                    if this_user.is_authenticated() {
                                        let navigate = use_navigate();
                                        navigate("/", NavigateOptions::default());
                                    }
                                    set_server_errors.set(None);
                                }
                                Err(error) => {
                                    set_server_errors.set(Some(error));
                                }
                            }
                        }} {move || view! { <ServerErrors server_errors=server_errors.get() /> }}
                    </Suspense>

                    <label class="flex gap-2 items-center input input-bordered">
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 16 16"
                            fill="currentColor"
                            class="w-4 h-4 opacity-70"
                        >
                            <path d="M8 8a3 3 0 1 0 0-6 3 3 0 0 0 0 6ZM12.735 14c.618 0 1.093-.561.872-1.139a6.002 6.002 0 0 0-11.215 0c-.22.578.254 1.139.872 1.139h9.47Z" />
                        </svg>
                        <input type="text" class="grow" placeholder="Username" name="username" />
                    </label>
                    <label class="flex gap-2 items-center input input-bordered">
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
                    <label class="flex gap-2 items-center input input-bordered">
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
                            placeholder="Password Confirmation"
                            name="password_confirmation"
                        />
                    </label>

                    <div class="form-control">
                        <label class="cursor-pointer label">
                            <span class="label-text">"Remember me"</span>
                            <input type="checkbox" name="remember" class="checkbox" />
                        </label>
                    </div>

                    <button class="btn btn-primary">"Register"</button>
                </div>
            </ActionForm>
        </article>
    }
}
