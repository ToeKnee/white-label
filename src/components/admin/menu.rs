//! Admin menu module

use leptos::prelude::*;
use leptos_router::components::A;

use crate::components::utils::{error::ErrorPage, loading::Loading};
use crate::models::artist::Artist;
use crate::routes::menu::{AdminMenu, MenuArtist, MenuPage, MenuRelease, get_admin_menu};

/// Admin menu component that provides navigation links for the admin section of the application.
#[component]
pub fn AdminMenu() -> impl IntoView {
    let menu = RwSignal::new(AdminMenu::default());
    let menu_resource = Resource::new(move || {}, |()| get_admin_menu());

    view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    match menu_resource.await {
                        Ok(menu_result) => {
                            menu.set(menu_result);
                        }
                        Err(e) => {
                            tracing::error!("Error: {e:?}");
                        }
                    }
                    view! {
                        <ul class="w-56 menu bg-base-200 rounded-box">
                            <li>
                                <A href="/admin">"Dashboard"</A>
                            </li>
                            <li>
                                {move || {
                                    view! {
                                        <A href=menu
                                            .get()
                                            .url>{move || menu.get().record_label.name}</A>
                                    }
                                }}
                            </li>
                            <li>
                                <ArtistsMenu artists=menu.get().artists />
                            </li>

                            <li>
                                <PagesMenu pages=menu.get().pages />
                            </li>
                        </ul>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}

#[component]
fn ArtistsMenu(artists: Vec<MenuArtist>) -> impl IntoView {
    let artists = RwSignal::new(artists);
    view! {
        <details open>
            <summary>
                <A href="/admin/artists">Artists</A>
            </summary>
            <ul>
                <Show
                    when=move || { !artists.get().is_empty() }
                    fallback=|| view! { <li>"No artists yet..."</li> }
                >
                    <For
                        each=move || artists.get()
                        key=|menu_artist| (
                            menu_artist.artist.slug.clone(),
                            menu_artist.artist.name.clone(),
                        )
                        let(menu_artist)
                    >
                        <li>
                            <ArtistMenuRow menu_artist />
                        </li>
                    </For>
                </Show>
                <li>
                    <A href="/admin/artist">"Create Artist"</A>
                </li>
            </ul>
        </details>
    }
}

#[component]
fn ArtistMenuRow(menu_artist: MenuArtist) -> impl IntoView {
    let artist = RwSignal::new(menu_artist.artist);
    let artist_url = RwSignal::new(menu_artist.url);
    let releases = RwSignal::new(menu_artist.releases);
    let releases_url = move || format!("/admin/artist/{}/releases", artist.get().slug);
    let create_release_url = move || format!("/admin/artist/{}/releases/new", artist.get().slug);

    view! {
        <details>
            <summary>
                {move || {
                    view! { <A href=artist_url.get()>{move || artist.get().name}</A> }
                }}
            </summary>
            <ul>
                <li>
                    {move || {
                        view! { <A href=releases_url()>"All Releases"</A> }
                    }}
                </li>

                <Show
                    when=move || { !releases.get().is_empty() }
                    fallback=move || {
                        view! {
                            <li>
                                <A href=create_release_url>"No releases yet..."</A>
                            </li>
                        }
                    }
                >
                    <For
                        each=move || releases.get()
                        key=|menu_release| (
                            menu_release.release.slug.clone(),
                            menu_release.release.name.clone(),
                        )
                        let(menu_release)
                    >
                        <li>
                            <ReleaseMenuRow menu_release artist=artist />
                        </li>
                    </For>
                </Show>
                <li>
                    <A href=create_release_url>"Create Release"</A>
                </li>
            </ul>
        </details>
    }
}

#[component]
fn ReleaseMenuRow(
    #[prop(into)] menu_release: MenuRelease,
    artist: RwSignal<Artist>,
) -> impl IntoView {
    let release = RwSignal::new(menu_release.release);
    let release_url = menu_release.url;

    let primary_release_icon = move || {
        if release.get().primary_artist_id == artist.get().id {
            view! { <span title="Primary artist">"●"</span> }.into_any()
        } else {
            view! { <span title="Featured artist">"○"</span> }.into_any()
        }
    };
    view! { <A href=release_url>{primary_release_icon}" "{move || release.get().name}</A> }
}

#[component]
fn PagesMenu(#[prop(into)] pages: Vec<MenuPage>) -> impl IntoView {
    let pages = RwSignal::new(pages);
    view! {
        <details>
            <summary>
                <A href="/admin/pages">Pages</A>
            </summary>
            <ul>
                <Show
                    when=move || { !pages.get().is_empty() }
                    fallback=|| view! { <li>"No pages yet..."</li> }
                >
                    <For
                        each=move || pages.get()
                        key=|menu_page| (menu_page.page.name.clone(), menu_page.page.slug.clone())
                        let(menu_page)
                    >
                        <li>
                            <A href=menu_page.url>{menu_page.page.name}</A>
                        </li>
                    </For>
                </Show>
                <li>
                    <A href="/admin/page">"Create Page"</A>
                </li>
            </ul>
        </details>
    }
}
