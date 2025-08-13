//! Simple delete release component for the admin panel
//!
//! This component is used to delete a release from the admin panel.
//! It will show a confirmation dialog before deleting the item.

use leptos::ev::MouseEvent;
use leptos::html;
use leptos::prelude::*;

use crate::components::utils::error::ServerErrors;
use crate::models::release::Release;
use crate::routes::release::DeleteRelease;

/// Renders the delete release component.
///
/// Arguments:
/// * `release` - The release to be deleted.
#[component]
pub fn DeleteRelease(
    /// The release to be deleted
    release: RwSignal<Release>,
) -> impl IntoView {
    let dialog_element: NodeRef<html::Dialog> = NodeRef::new();

    let on_click_show = move |ev: MouseEvent| {
        ev.prevent_default();
        if let Some(dialog) = dialog_element.get() {
            dialog.show();
        }
    };

    let update_release = ServerAction::<DeleteRelease>::new();
    let value = update_release.value();

    view! {
        <button class="btn btn-error" on:click=on_click_show>
            Delete
        </button>

        <dialog class="modal" node_ref=dialog_element>
            <div class="modal-box">
                <h3 class="text-lg font-bold">"Delete "{move || release.get().name}</h3>
                <p>"Are you sure you want to delete " {move || release.get().name} "?"</p>
                <p>"This action will be performed immediately."</p>
                <p>
                    "This will perform a soft delete. "{move || release.get().name}
                    " will be unavailable to non-admin users."
                </p>
                <div class="modal-action">
                    <ActionForm action=update_release>
                        {move || {
                            match value.get() {
                                Some(Ok(release_result)) => {
                                    if release_result.release.deleted_at.is_some() {
                                        release.set(release_result.release);
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
                        }} <input name="slug" type="hidden" value=move || release.get().slug />
                        <button class="btn btn-error">Delete</button>
                    </ActionForm>
                </div>
            </div>
        </dialog>
    }
}
