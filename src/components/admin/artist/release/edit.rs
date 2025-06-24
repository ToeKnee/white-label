//! Edit a release in an artist's discography

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;

use super::delete::DeleteRelease;
use crate::components::{
    admin::{
        artist::release::track::list::Tracks,
        shared::{
            artist_select::ArtistSelect, date_field::DateField, markdown_field::MarkdownField,
        },
    },
    files::upload::FileUploadWithProgress,
    utils::{
        error::ErrorPage, error::ServerErrors, loading::Loading,
        permissions::permission_or_redirect, success::Success,
    },
};
use crate::config::upload::UploadConfiguration;
use crate::models::{artist::Artist, release::Release};
use crate::routes::{
    artist::get_artist,
    release::{ReleaseResult, UpdateRelease, get_release},
};
use crate::utils::redirect::redirect;

fn artists_ids(artists: &[Artist]) -> Vec<i64> {
    artists.iter().map(|a| a.id).collect::<Vec<_>>()
}

/// Renders the edit release page.
#[component]
#[allow(clippy::too_many_lines)] // components are a pain to make smaller
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
    let artist_resource = Resource::new(
        move || artist_slug,
        |artist_slug| get_artist(artist_slug.get()),
    );

    let release = RwSignal::new(Release::default());
    let artists = RwSignal::new(Vec::new()); // Artists on the release
    let artist_ids = RwSignal::new(vec![]);
    let release_resource = Resource::new(
        move || (artist_slug, release_slug),
        |(artist_slug, release_slug)| get_release(artist_slug.get(), release_slug.get()),
    );

    let update_release = ServerAction::<UpdateRelease>::new();
    let value = Signal::derive(move || {
        update_release
            .value()
            .get()
            .unwrap_or_else(|| Ok(ReleaseResult::default()))
    });
    let success = RwSignal::new(false);

    let name = move || format!("{} - {}", release.get().name, artist.get().name);
    view! {
        <div class="flex gap-6 justify-around w-full">
            <div class="w-2/3">
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
                                    redirect("/admin/artists/");
                                }
                            }
                            match release_resource.await {
                                Ok(this_release) => {
                                    release.set(this_release.release);
                                    artists.set(this_release.artists.clone());
                                    artist_ids.set(artists_ids(&this_release.artists));
                                }
                                Err(_) => {
                                    redirect(
                                        &format!("/admin/artists/{}/releases", artist_slug.get()),
                                    );
                                }
                            }

                            view! {
                                <Title text=name />
                                <h1>{name}</h1>

                                <ActionForm action=update_release>
                                    <div class="grid gap-6">
                                        {move || {
                                            match value.get() {
                                                Ok(release_result) => {
                                                    let fresh_release = release_result.release;
                                                    let fresh_artists = release_result.artists;
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
                                                        if fresh_release != release.get() {
                                                            release.set(fresh_release);
                                                        }
                                                        if fresh_artists != artists.get() {
                                                            artists.set(fresh_artists.clone());
                                                            artist_ids.set(artists_ids(&fresh_artists));
                                                        }
                                                        if !success.get() {
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
                                        }}
                                        <Form release=release artist=artist artist_ids=artist_ids />
                                    </div>
                                </ActionForm>
                            }
                        })}
                    </ErrorBoundary>
                </Transition>
            </div>
            <div class="w-1/3">
                <Tracks />
            </div>
        </div>
    }
}

#[component]
fn Form(
    release: RwSignal<Release>,
    artist: RwSignal<Artist>,
    artist_ids: RwSignal<Vec<i64>>,
) -> impl IntoView {
    view! {
        <input
            type="text"
            class="hidden"
            name="form[label_id]"
            value=move || { artist.get().label_id }
        />
        <input type="text" class="hidden" name="form[slug]" value=move || { release.get().slug } />
        <div class="divider">Public</div>
        <label class="flex gap-2 items-center input">
            <input
                type="text"
                class="grow"
                placeholder="Release name"
                name="form[name]"
                value=move || release.get().name
            />
        </label>
        {move || {
            view! {
                <MarkdownField
                    title="Description".to_string()
                    field="form[description]".to_string()
                    markdown_text=release.get().description
                />
            }
        }}
        {move || {
            view! {
                <ArtistSelect
                    primary_artist_id=release.get().primary_artist_id
                    artist_ids=artist_ids
                    label_id=artist.get().label_id
                />
            }
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
                <DateField
                    title="Release Date".to_string()
                    field="form[release_date]"
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
                    field="form[published_at]"
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
