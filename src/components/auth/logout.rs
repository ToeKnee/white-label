//! Logout page component.
use leptos::form::ActionForm;
use leptos::prelude::*;

use crate::app::UserContext;
use crate::models::auth::User;
use crate::routes::auth::Logout;
use crate::utils::split_at_colon::split_at_colon;

/// Renders the logout page.
#[component]
pub fn Logout() -> impl IntoView {
    let logout = ServerAction::<Logout>::new();
    let value = logout.value();
    let user_context = expect_context::<UserContext>();

    view! {
        <article class="my-6 md:container md:mx-auto prose">
            <h1>Log out</h1>

            <div class="grid gap-6">
                <ActionForm action=logout>
                    <ErrorBoundary fallback=|errors| {
                        view! {
                            <div role="alert" class="alert alert-warning">
                                <svg
                                    xmlns="http://www.w3.org/2000/svg"
                                    class="w-6 h-6 stroke-current shrink-0"
                                    fill="none"
                                    viewBox="0 0 24 24"
                                >
                                    <path
                                        stroke-linecap="round"
                                        stroke-linejoin="round"
                                        stroke-width="2"
                                        d="M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z"
                                    />
                                </svg>
                                {move || {
                                    errors
                                        .get()
                                        .into_iter()
                                        .last()
                                        .map(|(_, e)| {
                                            view! { <span>{split_at_colon(&e.to_string()).1}</span> }
                                        })
                                }}
                            </div>
                        }
                    }>
                        {move || {
                            let this_user = match value.get() {
                                Some(Ok(user)) => user,
                                Some(Err(e)) => {
                                    tracing::error!("{e}");
                                    User::default()
                                }
                                None => User::default(),
                            };
                            user_context.1.set(this_user);
                        }}
                    </ErrorBoundary>

                    {move || {
                        if value
                            .get()
                            .is_some_and(|user| match user {
                                Ok(user) => user.is_anonymous(),
                                Err(e) => {
                                    tracing::error!("{e}");
                                    false
                                }
                            })
                        {
                            view! { Logged out }.into_any()
                        } else {
                            view! { <button class="btn btn-primary">Log out</button> }.into_any()
                        }
                    }}
                </ActionForm>
            </div>
        </article>
    }
}
