//! Footer component for the record label page.
//! This component displays the footer with links to services, legal information, and the record label's details.
use leptos::prelude::*;
use leptos_router::components::A;
use reactive_stores::Store;

use crate::components::utils::error::ErrorPage;
use crate::components::utils::loading::Loading;
use crate::store::GlobalState;
use crate::store::GlobalStateStoreFields;

/// Renders the footer for the application.
#[component]
pub fn LabelFooter() -> impl IntoView {
    let store = expect_context::<Store<GlobalState>>();
    view! {
        <Transition fallback=move || view! { <Loading /> }>
            <ErrorBoundary fallback=|_| { ErrorPage }>

                <footer class="p-10 footer bg-neutral text-neutral-content md:footer-horizontal">
                    <nav>
                        <h6 class="footer-title">Services</h6>
                        <A href="/pages/branding" attr:class="link link-hover">
                            Branding
                        </A>
                        <A href="/pages/design" attr:class="link link-hover">
                            Design
                        </A>
                        <A href="/pages/marketing" attr:class="link link-hover">
                            Marketing
                        </A>
                        <A href="/pages/advertisment" attr:class="link link-hover">
                            Advertisement
                        </A>
                    </nav>
                    <nav>
                        <h6 class="footer-title">{move || store.record_label().get().name}</h6>
                        <A href="/pages/about-us" attr:class="link link-hover">
                            About us
                        </A>
                        <A href="/pages/contact" attr:class="link link-hover">
                            Contact
                        </A>
                        <A href="/pages/press-kit" attr:class="link link-hover">
                            Press kit
                        </A>
                    </nav>
                    <nav>
                        <h6 class="footer-title">Legal</h6>
                        <A href="/pages/terms-of-use" attr:class="link link-hover">
                            Terms of use
                        </A>
                        <A href="/pages/privacy-policy" attr:class="link link-hover">
                            Privacy policy
                        </A>
                        <A href="/pages/cookies" attr:class="link link-hover">
                            Cookie policy
                        </A>
                    </nav>
                </footer>
            </ErrorBoundary>
        </Transition>
    }
}
