//! Simple restore release component for the admin panel
//!
//! This component is used to restore an release from the admin panel.
//! It will show a confirmation restore before deleting the item.

use leptos::ev::MouseEvent;
use leptos::html;
use leptos::prelude::*;

use crate::components::utils::error::ServerErrors;
use crate::models::release::Release;
use crate::routes::release::{ReleaseResult, RestoreRelease};

/// Renders the restore release component.
#[component]
pub fn RestoreRelease(
    /// The release to restore
    release: RwSignal<Release>,
) -> impl IntoView {
    let dialog_element: NodeRef<html::Dialog> = NodeRef::new();

    let on_click_show = move |ev: MouseEvent| {
        ev.prevent_default();
        if let Some(dialog) = dialog_element.get() {
            dialog.show();
        }
    };

    let update_release = ServerAction::<RestoreRelease>::new();
    let value = Signal::derive(move || {
        update_release.value().get().unwrap_or_else(|| {
            Ok(ReleaseResult {
                release: release.get(),
                artists: vec![],
                tracks: vec![],
            })
        })
    });

    view! {
        <button class="btn btn-secondary" on:click=on_click_show>
            "Restore"
        </button>

        <dialog class="modal" node_ref=dialog_element>
            <div class="modal-box">
                <h3 class="text-lg font-bold">"Restore "{move || release.get().name}</h3>
                <p>"Are you sure you want to restore " {move || release.get().name} "?"</p>
                <p>"This action will be performed immediately."</p>
                <p>
                    "This will restore. " {move || release.get().name}
                    " will be available to non-admin users."
                </p>
                <div class="modal-action">
                    <ActionForm action=update_release>
                        {move || {
                            match value.get() {
                                Ok(release_result) => {
                                    if release_result.release.deleted_at.is_none() {
                                        release.set(release_result.release);
                                        if let Some(dialog) = dialog_element.get() {
                                            dialog.close();
                                        }
                                    }

                                    view! { "" }
                                        .into_any()
                                }
                                Err(errors) => {
                                    view! { <ServerErrors server_errors=Some(errors) /> }.into_any()
                                }
                            }
                        }} <input name="slug" type="hidden" value=move || release.get().slug />
                        <button class="btn btn-secondary">Restore</button>
                    </ActionForm>
                </div>
            </div>
        </dialog>
    }
}
