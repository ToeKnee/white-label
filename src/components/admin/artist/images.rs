//! Edit an artists images

use leptos::prelude::*;
use leptos_meta::Title;
use reactive_stores::Store;

use crate::components::{
    files::upload::FileUploadWithProgress, utils::permissions::permission_or_redirect,
};
use crate::config::upload::UploadConfiguration;
use crate::store::{GlobalState, GlobalStateStoreFields};

/// Renders the edit artist image page.
#[component]
pub fn EditArtistImages() -> impl IntoView {
    Effect::new_isomorphic(move || {
        permission_or_redirect("label_owner", "/admin");
    });

    let store = expect_context::<Store<GlobalState>>();
    let artist = store.artist();

    view! {
        <Title text=move || format!("{} Images", artist.get().name) />
        <h1>{move || view! { {artist.get().name} }}" Images"</h1>

        <div class="grid gap-6">
            <div class="divider">Images</div>
            {move || {
                view! {
                    <FileUploadWithProgress
                        config=UploadConfiguration::Artist
                        slug=artist.get().slug
                    />
                }
            }}
            <img src=move || artist.get().primary_image_url() alt=move || artist.get().name />
        </div>
    }
}
