//! Simple restore track component for the admin panel
//!
//! This component is used to restore an track from the admin panel.
//! It will show a confirmation restore before deleting the item.

use leptos::ev::MouseEvent;
use leptos::html;
use leptos::prelude::*;

use crate::components::utils::error::ServerErrors;
use crate::models::track::Track;
use crate::routes::track::{RestoreTrack, TrackResult};

/// Renders the restore track component.
#[component]
pub fn RestoreTrack(
    /// The track to restore
    track: RwSignal<Track>,
) -> impl IntoView {
    let dialog_element: NodeRef<html::Dialog> = NodeRef::new();

    let on_click_show = move |ev: MouseEvent| {
        ev.prevent_default();
        if let Some(dialog) = dialog_element.get() {
            dialog.show();
        }
    };

    let update_track = ServerAction::<RestoreTrack>::new();
    let value = Signal::derive(move || {
        update_track.value().get().unwrap_or_else(|| {
            Ok(TrackResult {
                track: track.get(),
                artists: vec![],
                releases: vec![],
            })
        })
    });

    view! {
        <button class="btn btn-secondary" on:click=on_click_show>
            "Restore"
        </button>

        <dialog class="modal" node_ref=dialog_element>
            <div class="modal-box">
                <h3 class="text-lg font-bold">"Restore "{move || track.get().name}</h3>
                <p>"Are you sure you want to restore " {move || track.get().name} "?"</p>
                <p>"This action will be performed immediately."</p>
                <p>
                    "This will restore. " {move || track.get().name}
                    " will be available to non-admin users."
                </p>
                <div class="modal-action">
                    <ActionForm action=update_track>
                        {move || {
                            match value.get() {
                                Ok(track_result) => {
                                    if track_result.track.deleted_at.is_none() {
                                        track.set(track_result.track);
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
                        }} <input name="slug" type="hidden" value=move || track.get().slug />
                        <button class="btn btn-secondary">Restore</button>
                    </ActionForm>
                </div>
            </div>
        </dialog>
    }
}
