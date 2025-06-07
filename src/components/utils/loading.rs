//! A loading spinner component for a web application using the Leptos framework.

use leptos::prelude::*;

/// A loading spinner.
#[component]
pub fn Loading() -> impl IntoView {
    view! {
        <article class="md:container md:mx-auto prose">
            <span class="loading loading-ring loading-lg"></span>
        </article>
    }
}
