//! Simple delete artist component for the admin panel
//!
//! This component is used to delete an artist from the admin panel.
//! It will show a confirmation dialog before deleting the item.

use leptos::ev::MouseEvent;
use leptos::html;
use leptos::prelude::*;

use crate::components::utils::error::ServerErrors;
use crate::models::artist::Artist;
use crate::routes::artist::{ArtistResult, DeleteArtist};
use crate::utils::redirect::redirect;

/// Renders the delete artist component.
#[component]
pub fn DeleteArtist(artist: Artist) -> impl IntoView {
    let dialog_element: NodeRef<html::Dialog> = NodeRef::new();

    let on_click_show = move |ev: MouseEvent| {
        ev.prevent_default();
        if let Some(dialog) = dialog_element.get() {
            dialog.show();
        }
    };

    let update_artist = ServerAction::<DeleteArtist>::new();
    let value = Signal::derive(move || {
        update_artist
            .value()
            .get()
            .unwrap_or_else(|| Ok(ArtistResult::default()))
    });

    view! {
        <button class="btn btn-error" on:click=on_click_show>
            Delete
        </button>

        <dialog class="modal" node_ref=dialog_element>
            <div class="modal-box">
                <h3 class="text-lg font-bold">"Delete "{artist.name.clone()}</h3>
                <p>"Are you sure you want to delete " {artist.name.clone()} "?"</p>
                <p>"This action will be performed immediately."</p>
                <p>
                    "This will perform a soft delete. "{artist.name}
                    " will be unavailable to non-admin users."
                </p>
                <div class="modal-action">
                    <ActionForm action=update_artist>
                        {move || {
                            match value.get() {
                                Ok(artist_result) => {
                                    if artist_result.artist.deleted_at.is_some() {
                                        redirect("/admin/");
                                    }

                                    view! { "" }
                                        .into_any()
                                }
                                Err(errors) => {
                                    view! { <ServerErrors server_errors=Some(errors) /> }.into_any()
                                }
                            }
                        }} <input name="slug" type="hidden" value=artist.slug />
                        <button class="btn btn-error">Delete</button>
                    </ActionForm>
                </div>
            </div>
        </dialog>
    }
}
