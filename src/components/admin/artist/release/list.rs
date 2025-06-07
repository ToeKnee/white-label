//! List releases in an artist's discography

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;

use crate::components::{
    admin::artist::menu::{Menu, Page},
    utils::{
        error::ErrorPage, loading::Loading, permissions::permission_or_redirect,
        status_badge::StatusBadge,
    },
};
use crate::models::{artist::Artist, release::Release};
use crate::routes::{artist::get_artist, release::get_releases};

use crate::utils::redirect::redirect;

/// Renders the list releases page.
#[component]
#[allow(clippy::too_many_lines)]
pub fn Releases() -> impl IntoView {
    Effect::new_isomorphic(move || {
        permission_or_redirect("label_owner", "/admin");
    });

    let params = use_params_map();
    let slug = RwSignal::new(String::new());
    Effect::new_isomorphic(move || {
        let s = params.read().get("slug").unwrap_or_default();
        slug.set(s);
    });

    let artist = RwSignal::new(Artist::default());
    let artist_resource = Resource::new(move || slug, |slug| get_artist(slug.get()));

    let releases_resource = Resource::new(move || slug, |slug| get_releases(slug.get()));
    let (releases, set_releases) = signal(Vec::new());

    let title = RwSignal::new("Releases".to_string());

    view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    match artist_resource.await {
                        Ok(this_artist) => {
                            artist.set(this_artist.artist);
                            title.set(format!("{} Releases", artist.get().name));
                        }
                        _ => {
                            redirect("/admin/artists");
                        }
                    }
                    if let Ok(releases) = releases_resource.await {
                        set_releases.set(releases.releases);
                    } else {
                        tracing::error!("Error while getting releases");
                        redirect("/admin/artists");
                    }
                    let release_rows = releases
                        .get()
                        .into_iter()
                        .map(|release| {
                            view! { <ReleaseRow release=release artist_slug=slug.get() /> }
                        })
                        .collect::<Vec<_>>();
                    view! {
                        <Title text=title.get() />
                        <h1>{move || title.get()}</h1>

                        <Menu slug=slug selected=&Page::Releases />

                        <div class="overflow-x-auto">
                            <table class="table">
                                <thead>
                                    <tr>
                                        <th></th>
                                        <th>Name</th>
                                        <th>Tracks</th>
                                        <th>Release Date</th>
                                        <th></th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {if releases.get().is_empty() {
                                        view! {
                                            <tr>
                                                <td colspan="5">No releases found.</td>
                                            </tr>
                                        }
                                            .into_any()
                                    } else {
                                        view! { {release_rows} }.into_any()
                                    }} <tr>
                                        <td colspan="4"></td>
                                        <td>
                                            <a
                                                href=format!("/admin/artist/{}/releases/new", slug.get())
                                                class="btn btn-primary"
                                            >
                                                Add
                                            </a>
                                        </td>
                                    </tr>
                                </tbody>
                                <tfoot>
                                    <tr>
                                        <th></th>
                                        <th>Name</th>
                                        <th>Tracks</th>
                                        <th>Release Date</th>
                                        <th></th>
                                    </tr>
                                </tfoot>
                            </table>
                        </div>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}

#[component]
fn ReleaseRow(#[prop(into)] release: Release, artist_slug: String) -> impl IntoView {
    let release_date = release.release_date.map_or_else(
        || "Unreleased".to_string(),
        |date| date.format("%e %B %Y").to_string(),
    );

    view! {
        <tr>
            <th>
                <StatusBadge deleted_at=release.deleted_at published_at=release.published_at />
            </th>
            <td>
                <div class="flex gap-3 items-center">
                    <div class="avatar">
                        <div class="w-12 rounded-full not-prose">
                            <img
                                class="m-0"
                                src=release.primary_image_url()
                                alt=release.name.clone()
                            />
                        </div>
                    </div>
                    <div>
                        <div class="font-bold">
                            <a href=format!(
                                "/admin/artist/{}/release/{}",
                                artist_slug,
                                release.slug,
                            )>{release.name.clone()}</a>
                        </div>
                        <div class="text-sm opacity-50">
                            {release.catalogue_number.clone()} <br /> {release.slug.clone()}
                        </div>
                    </div>
                </div>
            </td>
            <td>0</td>
            <td>{release_date}</td>
            <td>
                <a
                    href=format!("/admin/artist/{}/release/{}", artist_slug, release.slug)
                    class="btn btn-primary"
                >
                    Edit
                </a>
            </td>
        </tr>
    }
}

#[component]
fn ArtistRowFallback() -> impl IntoView {
    view! {
        <tr>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
        </tr>
        <tr>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
        </tr>
        <tr>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
        </tr>
    }
}
