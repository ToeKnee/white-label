use leptos::prelude::*;
use reactive_stores::Store;

use crate::components::utils::error::ErrorPage;
use crate::components::utils::loading::Loading;
use crate::models::record_label::RecordLabel;
use crate::routes::record_label::get_record_label;
use crate::store::GlobalState;
use crate::store::GlobalStateStoreFields;

/// Renders the record label page.
#[component]
pub fn LabelFooter() -> impl IntoView {
    let store = expect_context::<Store<GlobalState>>();
    let record_label_resource = Resource::new(move || store.record_label().get(), |_| get_record_label());
    view! {
        <Transition fallback=move || view! { <Loading /> }>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    if store.record_label().get().id == 0 {
                        match record_label_resource.await {
                            Ok(label) => {
                                let store_record_label = store.record_label();
                                *store_record_label.write() = label.label.clone();
                                label.label
                            }
                            Err(_) => RecordLabel::default(),
                        };
                    }
                    let record_label = store.record_label().get();

                    view! {
                        <footer class="p-10 footer bg-neutral text-neutral-content md:footer-horizontal">
                            <nav>
                                <h6 class="footer-title">Services</h6>
                                <a class="link link-hover">Branding</a>
                                <a class="link link-hover">Design</a>
                                <a class="link link-hover">Marketing</a>
                                <a class="link link-hover">Advertisement</a>
                            </nav>
                            <nav>
                                <h6 class="footer-title">{record_label.name}</h6>
                                <a href="/pages/about-us" class="link link-hover">
                                    About us
                                </a>
                                <a href="/pages/contact" class="link link-hover">
                                    Contact
                                </a>
                                <a href="/pages/press-kit" class="link link-hover">
                                    Press kit
                                </a>
                            </nav>
                            <nav>
                                <h6 class="footer-title">Legal</h6>
                                <a class="link link-hover">Terms of use</a>
                                <a class="link link-hover">Privacy policy</a>
                                <a class="link link-hover">Cookie policy</a>
                            </nav>
                        </footer>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}
