//! Simple delete page component for the admin panel
//!
//! This component is used to delete an page from the admin panel.
//! It will show a confirmation dialog before deleting the item.

use leptos::ev::MouseEvent;
use leptos::html;
use leptos::prelude::*;

use crate::components::utils::error::ServerErrors;
use crate::models::page::Page;
use crate::routes::page::DeletePage;

/// Renders the delete page component.
#[component]
pub fn DeletePage(
    /// The page to delete
    page: RwSignal<Page>,
) -> impl IntoView {
    let dialog_element: NodeRef<html::Dialog> = NodeRef::new();

    let on_click_show = move |ev: MouseEvent| {
        ev.prevent_default();
        if let Some(dialog) = dialog_element.get() {
            dialog.show();
        }
    };

    let update_page = ServerAction::<DeletePage>::new();
    let value = update_page.value();

    view! {
        <button class="btn btn-error" on:click=on_click_show>
            Delete
        </button>

        <dialog class="modal" node_ref=dialog_element>
            <div class="modal-box">
                <h3 class="text-lg font-bold">"Delete "{move || page.get().name}</h3>
                <p>"Are you sure you want to delete " {move || page.get().name} "?"</p>
                <p>"This action will be performed immediately."</p>
                <p>
                    "This will perform a soft delete. "{move || page.get().name}
                    " will be unavailable to non-admin users."
                </p>
                <div class="modal-action">
                    <ActionForm action=update_page>
                        {move || {
                            match value.get() {
                                Some(Ok(page_result)) => {
                                    if page_result.page.deleted_at.is_some() {
                                        page.set(page_result.page);
                                        if let Some(dialog) = dialog_element.get() {
                                            dialog.close();
                                        }
                                    }

                                    view! { "" }
                                        .into_any()
                                }
                                Some(Err(errors)) => {
                                    view! { <ServerErrors server_errors=Some(errors) /> }.into_any()
                                }
                                None => view! { "" }.into_any(),
                            }
                        }} <input name="slug" type="hidden" value=move || page.get().slug />
                        <button class="btn btn-error">Delete</button>
                    </ActionForm>
                </div>
            </div>
        </dialog>
    }
}
