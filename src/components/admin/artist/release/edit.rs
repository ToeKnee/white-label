use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;

use super::super::menu::{Menu, Page};
use super::delete::DeleteRelease;
use crate::components::{
    admin::shared::{DateField, MarkdownField},
    files::upload::FileUploadWithProgress,
    utils::{error::ErrorPage, error::ServerErrors, loading::Loading, permissions::permission_or_redirect, success::Success},
};
use crate::config::upload::UploadConfiguration;
use crate::models::{artist::Artist, release::Release};
use crate::routes::{
    artist::get_artist,
    release::{ReleaseResult, UpdateRelease, get_release},
};
use crate::utils::redirect::redirect;

/// Renders the create artist page.
#[component]
pub fn EditRelease() -> impl IntoView {
    Effect::new_isomorphic(move || {
        permission_or_redirect("label_owner", "/admin");
    });

    let params = use_params_map();
    let artist_slug = RwSignal::new(String::new());
    let release_slug = RwSignal::new(String::new());
    Effect::new_isomorphic(move || {
        let s = params.read().get("slug").unwrap_or_default();
        artist_slug.set(s);
        let rs = params.read().get("release_slug").unwrap_or_default();
        release_slug.set(rs);
    });

    let artist = RwSignal::new(Artist::default());
    let artist_resource = Resource::new(move || artist_slug, |artist_slug| get_artist(artist_slug.get()));
    let artist_ids = RwSignal::new(String::new());
    Effect::new_isomorphic(move || {
        let artist_ids_str = [artist.get().id.to_string()];
        artist_ids.set(artist_ids_str.join(","));
    });

    let release = RwSignal::new(Release::default());
    let artists = RwSignal::new(Vec::new()); // Artists on the release
    let release_resource = Resource::new(
        move || (artist_slug, release_slug),
        |(artist_slug, release_slug)| get_release(artist_slug.get(), release_slug.get()),
    );

    let update_release = ServerAction::<UpdateRelease>::new();
    let value = Signal::derive(move || update_release.value().get().unwrap_or_else(|| Ok(ReleaseResult::default())));
    let success = RwSignal::new(false);

    let name = move || format!("{} - {}", release.get().name, artist.get().name);
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
                        Err(_) => {
                            redirect("/admin/artists/{artist_slug.get()}/releases");
                        }
                    }
                    match release_resource.await {
                        Ok(this_release) => {
                            release.set(this_release.release);
                            artists.set(this_release.artists);
                        }
                        Err(_) => redirect("/admin/artists/{artist_slug.get()}/releases"),
                    }

                    view! {
                        <Header name=name() artist_slug=artist_slug />

                        <ActionForm action=update_release>
                            <div class="grid gap-6">
                                {move || {
                                    match value.get() {
                                        Ok(release_result) => {
                                            let fresh_release = release_result.release;
                                            if fresh_release.id > 0 {
                                                if fresh_release.slug != release.get().slug {
                                                    redirect(
                                                        &format!(
                                                            "/admin/artist/{}/release/{}",
                                                            artist.get().slug,
                                                            fresh_release.slug,
                                                        ),
                                                    );
                                                }
                                                if !success.get() {
                                                    release.set(fresh_release);
                                                    success.set(true);
                                                }
                                            } else {
                                                success.set(false);
                                            }

                                            view! { "" }
                                                .into_any()
                                        }
                                        Err(errors) => {
                                            success.set(false);
                                            view! { <ServerErrors server_errors=Some(errors) /> }
                                                .into_any()
                                        }
                                    }
                                }}
                                {move || {
                                    view! {
                                        <Success
                                            message=format!("{} Updated!", release.get().name)
                                            show=success.get()
                                        />
                                    }
                                }} <Form release=release artist=artist artist_ids=artist_ids />
                            </div>
                        </ActionForm>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}

#[component]
fn Header(name: String, artist_slug: RwSignal<String>) -> impl IntoView {
    view! {
        <Title text=name.clone() />
        <h1>{name}</h1>

        <Menu slug=artist_slug selected=&Page::Releases />
    }
}

#[component]
fn Form(release: RwSignal<Release>, artist: RwSignal<Artist>, artist_ids: RwSignal<String>) -> impl IntoView {
    view! {
        <input
            type="text"
            class="hidden"
            name="release_form[label_id]"
            value=move || { artist.get().label_id }
        />
        <input
            type="text"
            class="hidden"
            name="release_form[artist_ids]"
            value=move || { artist_ids.get() }
        />
        <input
            type="text"
            class="hidden"
            name="release_form[slug]"
            value=move || { release.get().slug }
        />
        <div class="divider">Public</div>
        <label class="flex gap-2 items-center input input-bordered">
            <input
                type="text"
                class="grow"
                placeholder="Release name"
                name="release_form[name]"
                value=move || release.get().name
            />
        </label>
        {move || {
            view! {
                <MarkdownField
                    title="Description".to_string()
                    field="release_form[description]".to_string()
                    markdown_text=release.get().description
                />
            }
        }}
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
                <DateField
                    title="Release Date".to_string()
                    field="release_form[release_date]".to_string()
                    date=release.get().release_date
                />
            }
        }}

        <div class="divider">Images</div>
        {move || {
            view! {
                <FileUploadWithProgress
                    config=UploadConfiguration::Release
                    slug=release.get().slug
                />
            }
        }}
        <img src=move || release.get().primary_image_url() alt=move || release.get().name />

        <div class="divider">Private</div>
        {move || {
            view! {
                <DateField
                    title="Published At".to_string()
                    field="release_form[published_at]".to_string()
                    date=release.get().published_at
                />
            }
        }}
        <div class="flex flex-auto gap-6">
            <button class="flex-1 btn btn-primary">Update</button>
            {move || {
                view! { <DeleteRelease release=release.get() /> }
            }}
        </div>
    }
}
