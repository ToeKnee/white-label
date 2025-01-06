use leptos::prelude::*;
use leptos_router::{components::Outlet, components::Redirect};

use crate::app::UserContext;
use crate::components::utils::error::ErrorPage;
use crate::components::utils::loading::Loading;
use crate::models::auth::User;

/// Renders the record label page.
#[component]
pub fn AdminRoot() -> impl IntoView {
    let user_context = expect_context::<UserContext>();
    let (user, set_user) = signal(User::default());

    view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    set_user.set(user_context.0.get().clone());
                    if !user.get().permissions.contains("admin") {
                        view! { <Redirect path="/login" /> }.into_any()
                    } else {
                        view! {
                            <article class="md:container md:mx-auto prose">
                                <Outlet />
                            </article>
                        }
                            .into_any()
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}
