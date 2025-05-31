use leptos::prelude::*;
use reactive_stores::Store;

use crate::app::UserContext;
use crate::components::utils::error::ErrorPage;
use crate::components::utils::status_badge::StatusBadge;
use crate::models::auth::User;
use crate::models::page::Page;
use crate::routes::record_label::get_label_pages;
use crate::store::GlobalState;
use crate::store::GlobalStateStoreFields;

/// Renders the record label page.
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
        <div class="basis-1/2">
            <div class="overflow-x-auto shadow-xl not-prose card bg-neutral text-neutral-content bg-base-100">
                <div class="card-body">
                    <h2 class="card-title">Pages</h2>
                    <table class="table">
                        <thead>
                            <tr>
                                <th>Status</th>
                                <th>Slug</th>
                                <th>Name</th>
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
                                        let page_rows = pages
                                            .get()
                                            .into_iter()
                                            .map(|page| {

                                                view! { <PageRow page /> }
                                            })
                                            .collect::<Vec<_>>();
                                        view! {
                                            {if pages.get().is_empty() {
                                                view! {
                                                    <tr>
                                                        <td colspan="5">No pages found.</td>
                                                    </tr>
                                                }
                                                    .into_any()
                                            } else {
                                                view! { {page_rows} }.into_any()
                                            }}
                                            <tr>
                                                <td colspan="5"></td>
                                                <td>
                                                    {if user.get().permissions.contains("label_owner") {
                                                        view! {
                                                            <a href="/admin/page" class="btn btn-primary">
                                                                Add
                                                            </a>
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
        </div>
    }
}

#[component]
fn PageRow(#[prop(into)] page: Page) -> impl IntoView {
    view! {
        <tr>
            <td>
                <StatusBadge deleted_at=page.deleted_at published_at=page.published_at />
            </td>
            <td>{page.slug.clone()}</td>
            <td>{page.name.clone()}</td>
            <td>
                <a href=format!("/admin/page/{}", page.slug) class="btn btn-primary">
                    Edit
                </a>
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
            <td class="w-full h-4 skeleton"></td>
        </tr>
        <tr>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
        </tr>
        <tr>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>

        </tr>
    }
}
