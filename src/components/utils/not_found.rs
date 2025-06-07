//! A generic 404 not found page.

use leptos::prelude::*;

/// A generic 404 not found page.
#[component]
pub fn NotFound() -> impl IntoView {
    view! {
        <article class="md:container md:mx-auto prose">
            <div class="min-h-screen hero bg-base-200">
                <div class="text-center hero-content">
                    <div class="max-w-md">
                        <h1 class="text-5xl font-bold">Oh No...</h1>
                        <p class="py-6">"Sorry, we couldn't find what you were looking for!"</p>
                        <p class="py-6">"Maybe it's hiding."</p>
                    </div>
                </div>
            </div>
        </article>
    }
}
