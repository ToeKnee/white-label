use leptos::prelude::*;

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
