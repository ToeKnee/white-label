//! Page details component

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;
use markdown;

use crate::components::utils::error::ErrorPage;
use crate::components::utils::loading::Loading;
use crate::models::page::Page;
use crate::routes::page::get_page;

/// Renders the page.
#[component]
pub fn PageDetails() -> impl IntoView {
    let params = use_params_map();
    let slug = move || params.read().get("slug");

    let (page, set_page) = signal(Page::default());
    let page_resource = Resource::new(
        move || page.get(),
        move |_| slug().map_or_else(|| get_page(String::new()), get_page),
    );
    view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    match page_resource.await {
                        Ok(this_page) => {
                            *set_page.write() = this_page.page.clone();
                            this_page.page
                        }
                        Err(_) => Page::default(),
                    };

                    view! {
                        <Title text=page.get().name />
                        <article class="md:container md:mx-auto prose">
                            <h1>{page.get().name}</h1>
                            <div inner_html=markdown::to_html_with_options(
                                    &page.get().body,
                                    &markdown::Options::gfm(),
                                )
                                .unwrap_or_default() />
                        </article>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}
