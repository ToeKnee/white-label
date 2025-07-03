//! Edit Page component for the admin interface.
use leptos::prelude::*;
use leptos_meta::{Meta, Title, provide_meta_context};
use leptos_router::hooks::use_params_map;

use super::{delete::DeletePage, restore::RestorePage};
use crate::components::{
    admin::shared::{date_field::DateField, markdown_field::MarkdownField},
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
    let slug = move || params.read().get("slug");

    let page = RwSignal::new(Page::default());
    let page_resource = Resource::new(move || slug().unwrap_or_default(), get_page);
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
        <Meta name="description" content=move || page.get().description />

        <h1>"Edit "{move || view! { {page.get().name} }}</h1>

        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    match page_resource.await {
                        Ok(this_page) => {
                            page.set(this_page.page);
                        }
                        _ => {
                            redirect("/admin/pages");
                        }
                    }
                    view! {
                        <ActionForm action=update_page>
                            <div class="grid gap-6">
                                {move || {
                                    match value.get() {
                                        Ok(page_result) => {
                                            let fresh_page = page_result.page;
                                            if fresh_page.id > 0 {
                                                if fresh_page.slug != page.get().slug {
                                                    redirect(&format!("/admin/page/{}", fresh_page.slug));
                                                }
                                                if !success.get() {
                                                    page.set(fresh_page);
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
                                }} <Form page=page slug=slug().unwrap_or_default() />

                            </div>
                        </ActionForm>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}

#[component]
fn Form(page: RwSignal<Page>, slug: String) -> impl IntoView {
    view! {
        <input type="text" class="hidden" name="page_form[slug]" value=slug />
        <div class="divider">Public</div>
        <label class="flex gap-2 items-center input">
            <input
                type="text"
                class="grow"
                placeholder="Page name"
                name="page_form[name]"
                value=move || page.get().name
            />
        </label>
        <h2>Meta Description</h2>
        <textarea
            class="w-full textarea"
            rows="5"
            name="page_form[description]"
            placeholder="Meta Description\nA short description of the page used for search engines."
        >
            {move || page.get().description}
        </textarea>
        {move || {
            view! {
                <MarkdownField
                    title="Body".to_string()
                    field="page_form[body]".to_string()
                    markdown_text=page.get().body
                />
            }
        }}
        <div class="divider">Private</div>
        {move || {
            view! {
                <DateField
                    title="Published at".to_string()
                    field="page_form[published_at]"
                    date=page.get().published_at
                />
            }
        }}
        <div class="flex flex-auto gap-6">
            <button class="flex-1 btn btn-primary">Update</button>
            {move || {
                if page.get().deleted_at.is_some() {
                    view! { <RestorePage page=page /> }.into_any()
                } else {
                    view! { <DeletePage page=page /> }.into_any()
                }
            }}
        </div>
    }
}
