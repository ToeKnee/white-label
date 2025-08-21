//! Simple restore artist component for the admin panel
//!
//! This component is used to restore an artist from the admin panel.
//! It will show a confirmation restore before deleting the item.

use leptos::ev::MouseEvent;
use leptos::html;
use leptos::prelude::*;
use reactive_stores::{Store, Subfield};

use crate::components::utils::error::ServerErrors;
use crate::models::artist::Artist;
use crate::routes::artist::RestoreArtist;
use crate::store::GlobalState;

/// Renders the restore artist component.
#[component]
pub fn RestoreArtist(
    /// The artist to restore
    artist: Subfield<Store<GlobalState>, GlobalState, Artist>,
) -> impl IntoView {
    let dialog_element: NodeRef<html::Dialog> = NodeRef::new();

    let on_click_show = move |ev: MouseEvent| {
        ev.prevent_default();
        if let Some(dialog) = dialog_element.get() {
            dialog.show();
        }
    };

    let update_artist = ServerAction::<RestoreArtist>::new();
    let value = update_artist.value();

    view! {
        <button class="btn btn-secondary" on:click=on_click_show>
            "Restore"
        </button>

        <dialog class="modal" node_ref=dialog_element>
            <div class="modal-box">
                <h3 class="text-lg font-bold">"Restore "{move || artist.get().name}</h3>
                <p>"Are you sure you want to restore " {move || artist.get().name} "?"</p>
                <p>"This action will be performed immediately."</p>
                <p>
                    "This will restore. " {move || artist.get().name}
                    " will be available to non-admin users."
                </p>
                <div class="modal-action">
                    <ActionForm action=update_artist>
                        {move || {
                            match value.get() {
                                Some(Ok(artist_result)) => {
                                    if artist_result.artist.deleted_at.is_none() {
                                        artist.set(artist_result.artist);
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
                        }} <input name="slug" type="hidden" value=move || artist.get().slug />
                        <button class="btn btn-secondary">Restore</button>
                    </ActionForm>
                </div>
            </div>
        </dialog>
    }
}
