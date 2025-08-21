//! Artists table component.
use leptos::prelude::*;
use leptos_router::components::A;

use crate::app::UserContext;
use crate::components::utils::error::ErrorPage;
use crate::components::utils::status_badge::StatusBadge;
use crate::models::artist::Artist;
use crate::models::auth::User;
use crate::routes::record_label::get_label_artists;

/// Renders the artists table component.
#[component]
pub fn ArtistsTable() -> impl IntoView {
    let (artists, set_artists) = signal(vec![]);
    let artists_resource = Resource::new(move || (), |()| get_label_artists());

    let user_context = expect_context::<UserContext>();
    let (user, set_user) = signal(User::default());

    view! {
        <div class="overflow-x-auto shadow-xl grow not-prose card bg-neutral text-neutral-content">
            <div class="card-body">
                <h2 class="card-title">Artists</h2>
                <table class="table">
                    <thead>
                        <tr class="text-neutral-content">
                            <th>Name</th>
                            <th>Status</th>
                            <th>Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        <Transition fallback=ArtistRowFallback>
                            <ErrorBoundary fallback=|_| {
                                ErrorPage
                            }>
                                {move || Suspend::new(async move {
                                    set_user.set(user_context.0.get());
                                    match artists_resource.await {
                                        Ok(these_artists) => {
                                            (*set_artists.write()).clone_from(&these_artists.artists);
                                            these_artists.artists
                                        }
                                        Err(_) => vec![Artist::default()],
                                    };
                                    view! {
                                        <Show
                                            when=move || { !artists.get().is_empty() }
                                            fallback=|| {
                                                view! {
                                                    <tr>
                                                        <td colspan="3">No artists found.</td>
                                                    </tr>
                                                }
                                            }
                                        >
                                            <For
                                                each=move || artists.get()
                                                key=|artist| (artist.slug.clone(), artist.name.clone())
                                                let(artist)
                                            >
                                                <ArtistRow artist />
                                            </For>
                                        </Show>
                                        <tr>
                                            <td colspan="2"></td>
                                            <td>
                                                {if user.get().permissions.contains("label_owner") {
                                                    view! {
                                                        <A href="/admin/artist" attr:class="btn btn-primary">
                                                            Add
                                                        </A>
                                                    }
                                                        .into_any()
                                                } else {
                                                    view! { "" }.into_any()
                                                }}
                                            </td>
                                        </tr>
                                    }
                                })}
                            </ErrorBoundary>
                        </Transition>
                    </tbody>
                </table>
            </div>
        </div>
    }
}

#[component]
fn ArtistRow(#[prop(into)] artist: Artist) -> impl IntoView {
    view! {
        <tr>
            <td>{artist.name.clone()}</td>
            <td>
                <StatusBadge deleted_at=artist.deleted_at published_at=artist.published_at />
            </td>
            <td>
                <A href=format!("/admin/artist/{}", artist.slug) attr:class="btn btn-primary">
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
        </tr>
        <tr>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
        </tr>
        <tr>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
            <td class="w-full h-4 skeleton"></td>
        </tr>
    }
}
