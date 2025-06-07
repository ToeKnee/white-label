//! Simple delete release component for the admin panel
//!
//! This component is used to delete a release from the admin panel.
//! It will show a confirmation dialog before deleting the item.

use leptos::ev::MouseEvent;
use leptos::html;
use leptos::prelude::*;

use crate::components::utils::error::ServerErrors;
use crate::models::release::Release;
use crate::routes::release::{DeleteRelease, ReleaseResult};
use crate::utils::redirect::redirect;

/// Renders the delete release component.
///
/// Arguments:
/// * `release` - The release to be deleted.
#[component]
pub fn DeleteRelease(
    /// The release to be deleted
    release: Release,
) -> impl IntoView {
    let dialog_element: NodeRef<html::Dialog> = NodeRef::new();

    let on_click_show = move |ev: MouseEvent| {
        ev.prevent_default();
        if let Some(dialog) = dialog_element.get() {
            dialog.show();
        }
    };

    let update_release = ServerAction::<DeleteRelease>::new();
    let value = Signal::derive(move || {
        update_release
            .value()
            .get()
            .unwrap_or_else(|| Ok(ReleaseResult::default()))
    });

    view! {
        <button class="btn btn-error" on:click=on_click_show>
            Delete
        </button>

        <dialog class="modal" node_ref=dialog_element>
            <div class="modal-box">
                <h3 class="text-lg font-bold">"Delete "{release.name.clone()}</h3>
                <p>"Are you sure you want to delete " {release.name.clone()} "?"</p>
                <p>"This action will be performed immediately."</p>
                <p>
                    "This will perform a soft delete. "{release.name}
                    " will be unavailable to non-admin users."
                </p>
                <div class="modal-action">
                    <ActionForm action=update_release>
                        {move || {
                            match value.get() {
                                Ok(release_result) => {
                                    if release_result.release.deleted_at.is_some() {
                                        redirect("../../releases");
                                    }

                                    view! { "" }
                                        .into_any()
                                }
                                Err(errors) => {
                                    view! { <ServerErrors server_errors=Some(errors) /> }.into_any()
                                }
                            }
                        }} <input name="slug" type="hidden" value=release.slug />
                        <button class="btn btn-error">Delete</button>
                    </ActionForm>
                </div>
            </div>
        </dialog>
    }
}
