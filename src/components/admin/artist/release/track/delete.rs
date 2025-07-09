//! Simple delete track component for the admin panel
//!
//! This component is used to delete a track from the admin panel.
//! It will show a confirmation dialog before deleting the item.

use leptos::ev::MouseEvent;
use leptos::html;
use leptos::prelude::*;

use crate::components::utils::error::ServerErrors;
use crate::models::track::Track;
use crate::routes::track::{DeleteTrack, TrackResult};

/// Renders the delete track component.
///
/// Arguments:
/// * `track` - The track to be deleted.
#[component]
pub fn DeleteTrack(
    /// The track to be deleted
    track: RwSignal<Track>,
) -> impl IntoView {
    let dialog_element: NodeRef<html::Dialog> = NodeRef::new();

    let on_click_show = move |ev: MouseEvent| {
        ev.prevent_default();
        if let Some(dialog) = dialog_element.get() {
            dialog.show();
        }
    };

    let update_track = ServerAction::<DeleteTrack>::new();
    let value = Signal::derive(move || {
        update_track
            .value()
            .get()
            .unwrap_or_else(|| Ok(TrackResult::default()))
    });

    view! {
        <button class="btn btn-error" on:click=on_click_show>
            Delete
        </button>

        <dialog class="modal" node_ref=dialog_element>
            <div class="modal-box">
                <h3 class="text-lg font-bold">"Delete "{move || track.get().name}</h3>
                <p>"Are you sure you want to delete " {move || track.get().name} "?"</p>
                <p>"This action will be performed immediately."</p>
                <p>
                    "This will perform a soft delete. "{move || track.get().name}
                    " will be unavailable to non-admin users."
                </p>
                <div class="modal-action">
                    <ActionForm action=update_track>
                        {move || {
                            match value.get() {
                                Ok(track_result) => {
                                    if track_result.track.deleted_at.is_some() {
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
                        <button class="btn btn-error">Delete</button>
                    </ActionForm>
                </div>
            </div>
        </dialog>
    }
}
