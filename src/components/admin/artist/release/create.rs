//! Create a new release in an artist's discography

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;
use reactive_stores::Store;

use crate::components::{
    admin::shared::{
        artist_select::ArtistSelect, date_field::DateField, markdown_field::MarkdownField,
    },
    utils::{
        error::ErrorPage, error::ServerErrors, loading::Loading,
        permissions::permission_or_redirect,
    },
};
use crate::models::{artist::Artist, release::Release};
use crate::routes::{
    artist::get_artist,
    release::{CreateRelease, ReleaseResult},
};
use crate::store::{GlobalState, GlobalStateStoreFields};
use crate::utils::redirect::redirect;

/// Renders the create release page.
#[component]
pub fn CreateRelease() -> impl IntoView {
    Effect::new_isomorphic(move || {
        permission_or_redirect("label_owner", "/admin");
    });

    let store = expect_context::<Store<GlobalState>>();
    let record_label = move || store.record_label().get();

    let params = use_params_map();
    let artist = RwSignal::new(Artist::default());
    let artist_resource = Resource::new(
        move || params.read().get("slug").unwrap_or_default(),
        get_artist,
    );
    let artist_ids = RwSignal::new(vec![]);
    Effect::new_isomorphic(move || {
        artist_ids.set(vec![artist.get().id]);
    });

    let (release, _set_release) = signal(Release::default());
    let create_release = ServerAction::<CreateRelease>::new();
    let value = Signal::derive(move || {
        create_release
            .value()
            .get()
            .unwrap_or_else(|| Ok(ReleaseResult::default()))
    });

    view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    match artist_resource.await {
                        Ok(this_artist) => {
                            artist.set(this_artist.artist);
                        }
                        _ => {
                            redirect("/admin/artists");
                        }
                    }
                    view! {
                        <Title text="New Release" />
                        <h1>New Release</h1>

                        <ActionForm action=create_release>
                            <div class="grid gap-6">
                                {move || {
                                    match value.get() {
                                        Ok(release_result) => {
                                            let release = release_result.release;
                                            if release.id > 0 {
                                                redirect(
                                                    &format!(
                                                        "/admin/artist/{}/release/{}",
                                                        artist.get().slug,
                                                        release.slug,
                                                    ),
                                                );
                                            }

                                            view! { "" }
                                                .into_any()
                                        }
                                        Err(errors) => {
                                            view! { <ServerErrors server_errors=Some(errors) /> }
                                                .into_any()
                                        }
                                    }
                                }}
                                <input
                                    type="text"
                                    class="hidden"
                                    placeholder=""
                                    name="form[label_id]"
                                    value=move || { record_label().id }
                                /><Form release artist artist_ids />
                            </div>
                        </ActionForm>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}

#[component]
fn Form(
    release: ReadSignal<Release>,
    artist: RwSignal<Artist>,
    artist_ids: RwSignal<Vec<i64>>,
) -> impl IntoView {
    view! {
        <label class="flex gap-2 items-center input">
            <input
                type="text"
                class="grow"
                placeholder="Release name"
                name="form[name]"
                value=move || release.get().name
            />
        </label>
        <MarkdownField
            title="Description".to_string()
            field="form[description]".to_string()
            markdown_text=String::new()
        />
        {move || {
            view! { <ArtistSelect primary_artist_id=artist.get().id artist_ids=artist_ids /> }
        }}
        <label class="flex gap-2 items-center input">
            <input
                type="text"
                class="grow"
                placeholder="Catalog number"
                name="form[catalogue_number]"
                value=move || release.get().catalogue_number
            />
        </label>
        {move || {
            view! {
                <div class="flex gap-6">
                    <div class="w-1/2">
                        <DateField
                            title="Release Date".to_string()
                            field="form[release_date]"
                            date=release.get().release_date
                        />
                    </div>
                    <div class="w-1/2">
                        <DateField
                            title="Published At".to_string()
                            field="form[published_at]"
                            date=release.get().published_at
                        />
                    </div>
                </div>
            }
        }}
        <button class="btn btn-primary">Create</button>
    }
}
