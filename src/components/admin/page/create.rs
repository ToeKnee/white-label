use leptos::prelude::*;
use leptos_meta::Title;
use reactive_stores::Store;

use crate::components::{
    admin::shared::{date_field::DateField, markdown_field::MarkdownField},
    utils::{
        error::ErrorPage, error::ServerErrors, loading::Loading,
        permissions::permission_or_redirect,
    },
};
use crate::models::page::Page;
use crate::routes::page::{CreatePage, PageResult};
use crate::store::{GlobalState, GlobalStateStoreFields};
use crate::utils::redirect::redirect;

/// Renders the create page page.
#[component]
pub fn CreatePage() -> impl IntoView {
    Effect::new_isomorphic(move || {
        permission_or_redirect("label_owner", "/admin");
    });

    let store = expect_context::<Store<GlobalState>>();
    let (page, set_page) = signal(Page::default());

    // Set the record label id to the page
    Effect::new_isomorphic(move || {
        let mut a = page.get();
        if a.label_id == 0 && store.record_label().get().id > 0 {
            a.label_id = store.record_label().get().id;
            set_page.set(a);
        }
    });

    let create_page = ServerAction::<CreatePage>::new();
    let value = Signal::derive(move || {
        create_page
            .value()
            .get()
            .unwrap_or_else(|| Ok(PageResult::default()))
    });

    let var_name = view! {
        <Title text="Create Page" />
        <h1>Create Page</h1>

        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    view! {
                        <ActionForm action=create_page>
                            <div class="grid gap-6">
                                {move || {
                                    match value.get() {
                                        Ok(page_result) => {
                                            let page = page_result.page;
                                            if page.id > 0 {
                                                redirect(&format!("/admin/page/{}", page.slug));
                                            }

                                            view! { "" }
                                                .into_any()
                                        }
                                        Err(errors) => {
                                            view! { <ServerErrors server_errors=Some(errors) /> }
                                                .into_any()
                                        }
                                    }
                                }}
                                <input
                                    type="text"
                                    class="hidden"
                                    placeholder=""
                                    name="page_form[label_id]"
                                    value=move || store.record_label().get().id
                                /> <div class="divider">Public</div>
                                <label class="flex gap-2 items-center input">
                                    <input
                                        type="text"
                                        class="grow"
                                        placeholder="Page name"
                                        name="page_form[name]"
                                        value=move || page.get().name
                                    />
                                </label> <h2>Meta Description</h2>
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
                                }} <div class="divider">Private</div>
                                {move || {
                                    view! {
                                        <DateField
                                            title="Published at".to_string()
                                            field="page_form[published_at]"
                                            date=page.get().published_at
                                        />
                                    }
                                }} <button class="btn btn-primary">Create</button>
                            </div>
                        </ActionForm>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    };
    var_name
}
