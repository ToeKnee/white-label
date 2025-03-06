use leptos::prelude::*;
use leptos_meta::{Meta, Title, provide_meta_context};
use leptos_router::hooks::use_params_map;

use super::delete::DeletePage;
use crate::components::{
    admin::shared::{MarkdownField, PublishedAtField},
    utils::{
        error::ErrorPage, error::ServerErrors, loading::Loading,
        permissions::permission_or_redirect, success::Success,
    },
};
use crate::models::page::Page;
use crate::routes::page::{PageResult, UpdatePage, get_page};
use crate::utils::redirect::redirect;

/// Renders the create page page.
#[component]
pub fn EditPage() -> impl IntoView {
    provide_meta_context();

    Effect::new_isomorphic(move || {
        permission_or_redirect("label_owner", "/admin");
    });

    let params = use_params_map();
    let slug = RwSignal::new(params.read().get("slug").unwrap_or_default());

    let (page, set_page) = signal(Page::default());
    let page_resource = Resource::new(move || slug, |slug| get_page(slug.get()));
    let update_page = ServerAction::<UpdatePage>::new();
    let value = Signal::derive(move || {
        update_page
            .value()
            .get()
            .unwrap_or_else(|| Ok(PageResult::default()))
    });
    let (success, set_success) = signal(false);

    view! {
        <Title text=move || format!("Edit {}", page.get().name) />
        <Meta name="description" content=page.get().description />

        <h1>"Edit "{move || view! { {page.get().name} }}</h1>

        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    match page_resource.await {
                        Ok(this_page) => {
                            set_page.set(this_page.page);
                        }
                        _ => {
                            redirect("/admin/pages");
                        }
                    };
                    view! {
                        <ActionForm action=update_page>
                            <div class="grid gap-6">
                                {move || {
                                    match value.get() {
                                        Ok(page_result) => {
                                            let fresh_page = page_result.page;
                                            if fresh_page.id > 0 {
                                                if fresh_page.slug != page.get().slug {
                                                    set_page.set(fresh_page.clone());
                                                    slug.set(fresh_page.clone().slug);
                                                    redirect(&format!("/admin/page/{}", fresh_page.slug));
                                                }
                                                if !success.get() {
                                                    set_page.set(fresh_page);
                                                    set_success.set(true);
                                                }
                                            } else {
                                                set_success.set(false);
                                            }

                                            view! { "" }
                                                .into_any()
                                        }
                                        Err(errors) => {
                                            set_success.set(false);
                                            view! { <ServerErrors server_errors=Some(errors) /> }
                                                .into_any()
                                        }
                                    }
                                }}
                                {move || {
                                    view! {
                                        <Success
                                            message=format!("{} Updated!", page.get().name)
                                            show=success.get()
                                        />
                                    }
                                }} <Form page=page slug=slug />

                            </div>
                        </ActionForm>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}

#[component]
fn Form(page: ReadSignal<Page>, slug: RwSignal<String>) -> impl IntoView {
    view! {
        <input type="text" class="hidden" name="page_form[slug]" bind:value=slug />
        <div class="divider">Public</div>
        <label class="flex gap-2 items-center input input-bordered">
            <input
                type="text"
                class="grow"
                placeholder="Page name"
                name="page_form[name]"
                value=page.get().name
            />
        </label>
        <h2>Meta Description</h2>
        <textarea
            class="textarea textarea-bordered"
            rows="5"
            name="page_form[description]"
            placeholder="Meta Description\nA short description of the page used for search engines."
        >
            {page.get().description}
        </textarea>
        <MarkdownField
            title="Body".to_string()
            field="page_form[body]".to_string()
            markdown_text=page.get().body
        />
        <div class="divider">Private</div>
        {move || {
            view! {
                <PublishedAtField
                    field="page_form[published_at]".to_string()
                    published_at=page.get().published_at
                />
            }
        }}
        <div class="flex flex-auto gap-6">
            <button class="flex-1 btn btn-primary">Update</button>
            {move || {
                view! { <DeletePage page=page.get() /> }
            }}
        </div>
    }
}
