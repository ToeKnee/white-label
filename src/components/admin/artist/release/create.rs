use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;
use reactive_stores::Store;

use crate::components::{
    admin::{
        artist::menu::{Menu, Page},
        shared::{DateField, MarkdownField},
    },
    utils::{error::ErrorPage, error::ServerErrors, loading::Loading, permissions::permission_or_redirect},
};
use crate::models::{artist::Artist, release::Release};
use crate::routes::{
    artist::get_artist,
    release::{CreateRelease, ReleaseResult},
};

use crate::store::{GlobalState, GlobalStateStoreFields};
use crate::utils::redirect::redirect;

/// Renders the create artist page.
#[component]
pub fn CreateRelease() -> impl IntoView {
    Effect::new_isomorphic(move || {
        permission_or_redirect("label_owner", "/admin");
    });

    let store = expect_context::<Store<GlobalState>>();
    let record_label = move || store.record_label().get();

    let params = use_params_map();
    let slug = RwSignal::new(String::new());
    Effect::new_isomorphic(move || {
        let s = params.read().get("slug").unwrap_or_default();
        slug.set(s);
    });

    let (artist, set_artist) = signal(Artist::default());
    let artist_resource = Resource::new(move || slug, |slug| get_artist(slug.get()));
    let (artist_ids, set_artist_ids) = signal(String::new());
    Effect::new_isomorphic(move || {
        let artist_ids_str = [artist.get().id.to_string()];
        set_artist_ids.set(artist_ids_str.join(","));
    });

    let (release, _set_release) = signal(Release::default());
    let create_release = ServerAction::<CreateRelease>::new();
    let value = Signal::derive(move || create_release.value().get().unwrap_or_else(|| Ok(ReleaseResult::default())));

    view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    match artist_resource.await {
                        Ok(this_artist) => {
                            set_artist.set(this_artist.artist);
                        }
                        _ => {
                            redirect("/admin/artists");
                        }
                    }
                    view! {
                        <Title text="New Release" />
                        <h1>New Release</h1>

                        <Menu slug=slug selected=&Page::Releases />

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
                                    name="release_form[label_id]"
                                    value=move || { record_label().id }
                                />
                                <input
                                    type="text"
                                    class="hidden"
                                    placeholder=""
                                    name="release_form[artist_ids]"
                                    value=move || { artist_ids.get() }
                                /> <Form release=release />
                            </div>
                        </ActionForm>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}

#[component]
fn Form(release: ReadSignal<Release>) -> impl IntoView {
    view! {
        <label class="flex gap-2 items-center input input-bordered">
            <input
                type="text"
                class="grow"
                placeholder="Release name"
                name="release_form[name]"
                value=move || release.get().name
            />
        </label>
        <MarkdownField
            title="Description".to_string()
            field="release_form[description]".to_string()
            markdown_text=String::new()
        />
        <label class="flex gap-2 items-center input input-bordered">
            <input
                type="text"
                class="grow"
                placeholder="Catalog number"
                name="release_form[catalogue_number]"
                value=move || release.get().catalogue_number
            />
        </label>
        {move || {
            view! {
                <div class="flex gap-6">
                    <div class="w-1/2">
                        <DateField
                            title="Release Date".to_string()
                            field="release_form[release_date]".to_string()
                            date=release.get().release_date
                        />
                    </div>
                    <div class="w-1/2">
                        <DateField
                            title="Published At".to_string()
                            field="release_form[published_at]".to_string()
                            date=release.get().published_at
                        />
                    </div>
                </div>
            }
        }}
        <button class="btn btn-primary">Create</button>
    }
}
