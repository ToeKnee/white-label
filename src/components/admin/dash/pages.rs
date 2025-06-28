//! This module defines the `PagesTable` component, which displays a table of pages
use leptos::prelude::*;
use leptos_router::components::A;
use reactive_stores::Store;

use crate::app::UserContext;
use crate::components::utils::{error::ErrorPage, status_badge::StatusBadge};
use crate::models::auth::User;
use crate::store::{GlobalState, GlobalStateStoreFields};
use crate::{models::page::Page, routes::record_label::get_label_pages};

/// Renders the page list component.
#[component]
pub fn PagesTable() -> impl IntoView {
    let store = expect_context::<Store<GlobalState>>();

    let (pages, set_pages) = signal(vec![]);
    let pages_resource = Resource::new(
        move || store.record_label().get(),
        move |_| get_label_pages(store.record_label().get().id),
    );

    let user_context = expect_context::<UserContext>();
    let (user, set_user) = signal(User::default());

    view! {
        <div class="overflow-x-auto shadow-xl grow not-prose card bg-neutral text-neutral-content bg-base-100">
            <div class="card-body">
                <h2 class="card-title">Pages</h2>
                <table class="table">
                    <thead>
                        <tr>
                            <th>Name</th>
                            <th>Status</th>
                            <th>Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        <Transition fallback=PageRowFallback>
                            <ErrorBoundary fallback=|_| {
                                ErrorPage
                            }>
                                {move || Suspend::new(async move {
                                    set_user.set(user_context.0.get());
                                    match pages_resource.await {
                                        Ok(these_pages) => {
                                            (*set_pages.write()).clone_from(&these_pages.pages);
                                            these_pages.pages
                                        }
                                        Err(_) => vec![Page::default()],
                                    };

                                    view! {
                                        <Show
                                            when=move || { !pages.get().is_empty() }
                                            fallback=|| {
                                                view! {
                                                    <tr>
                                                        <td colspan="3">No pages found.</td>
                                                    </tr>
                                                }
                                            }
                                        >
                                            <For
                                                each=move || pages.get()
                                                key=|page| (page.slug.clone(), page.name.clone())
                                                let(page)
                                            >
                                                <PageRow page />
                                            </For>
                                        </Show>
                                        <tr>
                                            <td colspan="2"></td>
                                            <td>
                                                {if user.get().permissions.contains("label_owner") {
                                                    view! {
                                                        <A href="/admin/page" attr:class="btn btn-primary">
                                                            Add
                                                        </A>
                                                    }
                                                        .into_any()
                                                } else {
                                                    view! { "" }.into_any()
                                                }}
                                            </td>
                                        </tr>
                                    }
                                })}
                            </ErrorBoundary>
                        </Transition>
                    </tbody>
                </table>
            </div>
        </div>
    }
}

#[component]
fn PageRow(#[prop(into)] page: Page) -> impl IntoView {
    view! {
        <tr>
            <td>{page.name.clone()}</td>
            <td>
                <StatusBadge deleted_at=page.deleted_at published_at=page.published_at />
            </td>
            <td>
                <A href=format!("/admin/page/{}", page.slug) attr:class="btn btn-primary">
                    Edit
                </A>
            </td>
        </tr>
    }
}

#[component]
fn PageRowFallback() -> impl IntoView {
    view! {
        <tr>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
        </tr>
        <tr>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
        </tr>
        <tr>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
        </tr>
    }
}
