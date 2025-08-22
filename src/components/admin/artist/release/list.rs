//! List releases in an artist's discography

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::{components::A, hooks::use_params_map};
use reactive_stores::Store;

use crate::components::utils::{
    error::ErrorPage, loading::Loading, permissions::permission_or_redirect,
    status_badge::StatusBadge,
};
use crate::models::release::Release;
use crate::routes::release::get_releases;
use crate::store::{GlobalState, GlobalStateStoreFields};

/// Renders the list releases page.
#[component]
#[allow(clippy::too_many_lines)]
pub fn Releases() -> impl IntoView {
    Effect::new_isomorphic(move || {
        permission_or_redirect("label_owner", "/admin");
    });

    let params = use_params_map();
    let store = expect_context::<Store<GlobalState>>();
    let artist = store.artist();

    let releases_resource = Resource::new(
        move || params.read().get("artist_slug").unwrap_or_default(),
        get_releases,
    );
    let releases = RwSignal::new(Vec::new());

    view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    if !artist.get().slug.is_empty() {
                        match releases_resource.await {
                            Ok(releases_result) => {
                                releases.set(releases_result.releases);
                            }
                            Err(e) => {
                                tracing::error!("Error while getting releases: {}", e);
                            }
                        }
                    }
                })} <Title text=move || format!("{} Releases", artist.get().name) />
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
                                    <ReleaseRow release=release />
                                </For>
                            </Show>
                            <tr>
                                <td colspan="4"></td>
                                <td>
                                    <A
                                        href=move || {
                                            format!("/admin/artist/{}/releases/new", artist.get().slug)
                                        }
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

            </ErrorBoundary>
        </Transition>
    }
}

#[component]
fn ReleaseRow(#[prop(into)] release: Release) -> impl IntoView {
    let store = expect_context::<Store<GlobalState>>();
    let artist = store.artist();

    let release_date = release.release_date.map_or_else(
        || "Unreleased".to_string(),
        |date| date.format("%e %B %Y").to_string(),
    );
    let release_slug = release.slug.clone();
    let release_url = move || {
        format!(
            "/admin/artist/{}/release/{}",
            artist.get().slug,
            release_slug,
        )
    };

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
                            <A href=release_url>{release.name.clone()}</A>
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
                    href=move || {
                        format!("/admin/artist/{}/release/{}", artist.get().slug, release.slug)
                    }
                    attr:class="btn btn-primary"
                >
                    Edit
                </A>
            </td>
        </tr>
    }
}
