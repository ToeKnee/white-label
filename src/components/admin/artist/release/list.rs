//! List releases in an artist's discography

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::{components::A, hooks::use_params_map};

use crate::components::utils::{
    error::ErrorPage, loading::Loading, permissions::permission_or_redirect,
    status_badge::StatusBadge,
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

    let artist = RwSignal::new(Artist::default());
    let artist_resource = Resource::new(
        move || params.read().get("slug").unwrap_or_default(),
        get_artist,
    );

    let releases_resource = Resource::new(
        move || params.read().get("slug").unwrap_or_default(),
        get_releases,
    );
    let (releases, set_releases) = signal(Vec::new());

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
                    if !params.read().get("slug").unwrap_or_default().is_empty() {
                        if let Ok(releases) = releases_resource.await {
                            set_releases.set(releases.releases);
                        } else {
                            tracing::error!("Error while getting releases");
                            redirect("/admin/artists");
                        }
                    }

                    view! {
                        <Title text=move || format!("{} Releases", artist.get().name) />
                        <h1>{move || format!("{} Releases", artist.get().name)}</h1>

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
                                    <Show
                                        when=move || { !releases.get().is_empty() }
                                        fallback=|| {
                                            view! {
                                                <tr>
                                                    <td colspan="5">No releases found.</td>
                                                </tr>
                                            }
                                        }
                                    >
                                        <For
                                            each=move || releases.get()
                                            key=|release| (release.slug.clone(), release.name.clone())
                                            let(release)
                                        >
                                            <ReleaseRow
                                                release=release
                                                artist_slug=params.read().get("slug").unwrap_or_default()
                                            />
                                        </For>
                                    </Show>
                                    <tr>
                                        <td colspan="4"></td>
                                        <td>
                                            <A
                                                href=format!(
                                                    "/admin/artist/{}/releases/new",
                                                    params.read().get("slug").unwrap_or_default(),
                                                )
                                                attr:class="btn btn-primary"
                                            >
                                                Add
                                            </A>
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
                            <A href=format!(
                                "/admin/artist/{}/release/{}",
                                artist_slug,
                                release.slug,
                            )>{release.name.clone()}</A>
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
                <A
                    href=format!("/admin/artist/{}/release/{}", artist_slug, release.slug)
                    attr:class="btn btn-primary"
                >
                    Edit
                </A>
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
