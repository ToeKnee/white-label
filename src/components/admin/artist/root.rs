//! The admin root component for the application.

use leptos::prelude::*;
use leptos_router::{
    components::{A, Outlet},
    hooks::{use_params_map, use_url},
};
use reactive_stores::Store;

use crate::components::utils::{error::ErrorPage, loading::Loading};
use crate::routes::artist::get_artist;
use crate::store::{GlobalState, GlobalStateStoreFields};
use crate::utils::redirect::redirect;

/// Renders the record label page.
#[component]
#[allow(clippy::too_many_lines)] // components are a pain to make smaller
pub fn AdminArtistRoot() -> impl IntoView {
    let store = expect_context::<Store<GlobalState>>();
    let params = use_params_map();
    let url = use_url();

    let artist_resource = Resource::new(
        move || params.read().get("slug").unwrap_or_default(),
        get_artist,
    );

    view! {
        <div role="tablist" class="mb-6 tabs tabs-border not-prose">
            <A
                href=move || {
                    format!("/admin/artist/{}", params.read().get("slug").unwrap_or_default())
                }
                attr:role="tab"
                attr:class=move || {
                    if url.get().path()
                        == format!(
                            "/admin/artist/{}",
                            params.read().get("slug").unwrap_or_default(),
                        )
                    {
                        "tab tab-active"
                    } else {
                        "tab"
                    }
                }
            >
                Profile
            </A>
            <A
                href=move || {
                    format!(
                        "/admin/artist/{}/releases",
                        params.read().get("slug").unwrap_or_default(),
                    )
                }
                attr:role="tab"
                attr:class=move || {
                    if url.get().path()
                        == format!(
                            "/admin/artist/{}/releases",
                            params.read().get("slug").unwrap_or_default(),
                        )
                    {
                        "tab tab-active"
                    } else {
                        "tab"
                    }
                }
            >
                Releases
            </A>
            <A
                href=move || {
                    format!("/admin/artist/{}/links", params.read().get("slug").unwrap_or_default())
                }
                attr:role="tab"
                attr:class=move || {
                    if url.get().path()
                        == format!(
                            "/admin/artist/{}/links",
                            params.read().get("slug").unwrap_or_default(),
                        )
                    {
                        "tab tab-active"
                    } else {
                        "tab"
                    }
                }
            >
                Links
            </A>
            <A
                href=move || {
                    format!(
                        "/admin/artist/{}/images",
                        params.read().get("slug").unwrap_or_default(),
                    )
                }
                attr:role="tab"
                attr:class=move || {
                    if url.get().path()
                        == format!(
                            "/admin/artist/{}/images",
                            params.read().get("slug").unwrap_or_default(),
                        )
                    {
                        "tab tab-active"
                    } else {
                        "tab"
                    }
                }
            >
                Images
            </A>
        </div>

        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    if !params.read().get("slug").unwrap_or_default().is_empty() {
                        match artist_resource.await {
                            Ok(this_artist) => {
                                store.artist().set(Some(this_artist.artist));
                            }
                            _ => {
                                redirect("/admin/artists");
                            }
                        }
                    }
                })} <Outlet />
            </ErrorBoundary>
        </Transition>
    }
}
