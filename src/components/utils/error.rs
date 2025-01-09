use leptos::prelude::*;

use crate::utils::split_at_colon;

/// A generic error page.
#[component]
pub fn ErrorPage() -> impl IntoView {
    view! {
        <article class="md:container md:mx-auto prose">
            <div class="min-h-screen hero bg-base-200">
                <div class="text-center hero-content">
                    <div class="max-w-md">
                        <h1 class="text-5xl font-bold">Oh No...</h1>
                        <p class="py-6">"Something went wrong. Please try again later."</p>
                    </div>
                </div>
            </div>
        </article>
    }
}

/// Inline error message.
#[component]
pub fn InlineError(message: String) -> impl IntoView {
    view! {
        <div role="alert" class="alert alert-error">
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
            {message}
        </div>
    }
}

/// Display ServerFnError as an inline error message.
#[component]
pub fn ServerErrors(server_errors: Option<ServerFnError>) -> impl IntoView {
    server_errors.map_or_else(
        || view! { "" }.into_any(),
        |errors| view! { <InlineError message=split_at_colon(&errors.to_string()).1 /> }.into_any(),
    )
}
