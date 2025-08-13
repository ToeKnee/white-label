//! Simple restore page component for the admin panel
//!
//! This component is used to restore an page from the admin panel.
//! It will show a confirmation restore before deleting the item.

use leptos::ev::MouseEvent;
use leptos::html;
use leptos::prelude::*;

use crate::components::utils::error::ServerErrors;
use crate::models::page::Page;
use crate::routes::page::RestorePage;

/// Renders the restore page component.
#[component]
pub fn RestorePage(
    /// The page to restore
    page: RwSignal<Page>,
) -> impl IntoView {
    let dialog_element: NodeRef<html::Dialog> = NodeRef::new();

    let on_click_show = move |ev: MouseEvent| {
        ev.prevent_default();
        if let Some(dialog) = dialog_element.get() {
            dialog.show();
        }
    };

    let update_page = ServerAction::<RestorePage>::new();
    let value = update_page.value();

    view! {
        <button class="btn btn-secondary" on:click=on_click_show>
            "Restore"
        </button>

        <dialog class="modal" node_ref=dialog_element>
            <div class="modal-box">
                <h3 class="text-lg font-bold">"Restore "{move || page.get().name}</h3>
                <p>"Are you sure you want to restore " {move || page.get().name} "?"</p>
                <p>"This action will be performed immediately."</p>
                <p>
                    "This will restore. " {move || page.get().name}
                    " will be available to non-admin users."
                </p>
                <div class="modal-action">
                    <ActionForm action=update_page>
                        {move || {
                            match value.get() {
                                Some(Ok(page_result)) => {
                                    if page_result.page.deleted_at.is_none() {
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
                        <button class="btn btn-secondary">Restore</button>
                    </ActionForm>
                </div>
            </div>
        </dialog>
    }
}
