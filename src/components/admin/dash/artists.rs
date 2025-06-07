//! Artists table component.
use leptos::prelude::*;
use reactive_stores::Store;

use crate::app::UserContext;
use crate::components::utils::error::ErrorPage;
use crate::components::utils::status_badge::StatusBadge;
use crate::models::artist::Artist;
use crate::models::auth::User;
use crate::routes::record_label::get_label_artists;
use crate::store::GlobalState;
use crate::store::GlobalStateStoreFields;

/// Renders the artists table component.
#[component]
pub fn ArtistsTable() -> impl IntoView {
    let store = expect_context::<Store<GlobalState>>();

    let (artists, set_artists) = signal(vec![]);
    let artists_resource = Resource::new(
        move || store.record_label().get(),
        move |_| get_label_artists(store.record_label().get().id),
    );

    let user_context = expect_context::<UserContext>();
    let (user, set_user) = signal(User::default());

    view! {
        <div class="overflow-x-auto shadow-xl grow not-prose card bg-neutral text-neutral-content bg-base-100">
            <div class="card-body">
                <h2 class="card-title">Artists</h2>
                <table class="table">
                    <thead>
                        <tr>
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
                                    let artist_rows = artists
                                        .get()
                                        .into_iter()
                                        .map(|artist| {
                                            view! { <ArtistRow artist /> }
                                        })
                                        .collect::<Vec<_>>();
                                    view! {
                                        {if artists.get().is_empty() {
                                            view! {
                                                <tr>
                                                    <td colspan="3">No artists found.</td>
                                                </tr>
                                            }
                                                .into_any()
                                        } else {
                                            view! { {artist_rows} }.into_any()
                                        }}
                                        <tr>
                                            <td colspan="2"></td>
                                            <td>
                                                {if user.get().permissions.contains("label_owner") {
                                                    view! {
                                                        <a href="/admin/artist" class="btn btn-primary">
                                                            Add
                                                        </a>
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
                <a href=format!("/admin/artist/{}", artist.slug) class="btn btn-primary">
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
