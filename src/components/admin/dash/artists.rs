use leptos::prelude::*;
use reactive_stores::Store;

use crate::app::UserContext;
use crate::components::utils::error::ErrorPage;
use crate::components::utils::loading::Loading;
use crate::models::artist::Artist;
use crate::models::auth::User;
use crate::routes::record_label::get_label_artists;
use crate::store::GlobalState;
use crate::store::GlobalStateStoreFields;

/// Renders the record label page.
#[component]
pub fn ArtistsTable() -> impl IntoView {
    let store = expect_context::<Store<GlobalState>>();
    let (record_label, _set_record_label) = signal(store.record_label().get());

    let (artists, set_artists) = signal(store.artists().get());
    let artists_resource = Resource::new(
        move || artists.get(),
        move |_artists| get_label_artists(record_label.get().id),
    );

    let user_context = expect_context::<UserContext>();
    let (_user, set_user) = signal(User::default());

    view! {
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    set_user.set(user_context.0.get());
                    set_artists.set(store.artists().get());
                    if store.artists().get().is_empty() {
                        match artists_resource.await {
                            Ok(these_artists) => {
                                let artists = store.artists();
                                (*artists.write()).clone_from(&these_artists.artists);
                                (*set_artists.write()).clone_from(&these_artists.artists);
                                these_artists.artists
                            }
                            Err(_) => vec![Artist::default()],
                        };
                    }
                    let artist_rows = artists
                        .get()
                        .into_iter()
                        .map(|artist| {
                            view! { <ArtistRow artist /> }
                        })
                        .collect::<Vec<_>>();
                    view! {
                        <div class="basis-1/2">
                            <div class="overflow-x-auto shadow-xl card bg-base-100">
                                <div class="card-body">
                                    <h2 class="card-title">Artists</h2>
                                    <table class="table">
                                        <thead>
                                            <tr>
                                                <th></th>
                                                <th>Name</th>
                                                <th>Releases</th>
                                                <th>Tracks</th>
                                                <th>Actions</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {if artists.get().is_empty() {
                                                view! {
                                                    <tr>
                                                        <td colspan="5">No artists found.</td>
                                                    </tr>
                                                }
                                                    .into_any()
                                            } else {
                                                view! { {artist_rows} }.into_any()
                                            }}
                                        </tbody>
                                    </table>
                                </div>
                            </div>
                        </div>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    }
}

#[component]
fn ArtistRow(#[prop(into)] artist: Artist) -> impl IntoView {
    view! {
        <tr>
            <td>{artist.slug.clone()}</td>
            <td>{artist.name.clone()}</td>
            <td>0</td>
            <td>0</td>
            <td>
                <a href=format!("/admin/artist/{}", artist.slug) class="btn btn-primary">
                    Edit
                </a>
            </td>
        </tr>
    }
}
