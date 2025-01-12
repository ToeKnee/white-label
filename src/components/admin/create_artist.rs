use leptos::prelude::*;
use leptos_router::{hooks::use_navigate, NavigateOptions};
use reactive_stores::Store;

use crate::app::UserContext;
use crate::components::utils::{error::ErrorPage, error::ServerErrors, loading::Loading};
use crate::models::artist::Artist;
use crate::routes::artist::{ArtistResult, CreateArtist};
use crate::store::{GlobalState, GlobalStateStoreFields};

/// Renders the create artist page.
#[component]
pub fn CreateArtist() -> impl IntoView {
    let store = expect_context::<Store<GlobalState>>();
    let (record_label, _set_record_label) = signal(store.record_label().get());

    let user_context = expect_context::<UserContext>();
    let (user, _set_user) = signal(user_context.0.get());

    let (artist, set_artist) = signal(Artist::default());

    // Set the record label id to the artist
    Effect::new_isomorphic(move || {
        let mut a = artist.get();
        if a.label_id == 0 && record_label.get().id > 0 {
            a.label_id = record_label.get().id;
            set_artist.set(a);
        }
    });

    let create_artist = ServerAction::<CreateArtist>::new();
    let value = Signal::derive(move || {
        create_artist
            .value()
            .get()
            .unwrap_or_else(|| Ok(ArtistResult::default()))
    });

    Effect::new_isomorphic(move || {
        if user.get().is_active() && !user.get().permissions.contains("label_owner") {
            let navigate = use_navigate();
            navigate("/", NavigateOptions::default());
        }
    });

    let var_name = view! {
        <h1>Create Artist</h1>

        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    view! {
                        <ActionForm action=create_artist>
                            <div class="grid gap-6">
                                {move || {
                                    match value.get() {
                                        Ok(artist_result) => {
                                            let artist = artist_result.artist;
                                            if artist.id > 0 {
                                                let navigate = use_navigate();
                                                navigate(
                                                    &format!("/admin/artist/{}", artist.slug),
                                                    NavigateOptions::default(),
                                                );
                                            }

                                            view! { "" }
                                                .into_any()
                                        }
                                        Err(errors) => {
                                            view! { <ServerErrors server_errors=Some(errors) /> }
                                                .into_any()
                                        }
                                    }
                                }}
                                <input
                                    type="text"
                                    class="hidden"
                                    placeholder=""
                                    name="record_label_id"
                                    value=record_label.get().id
                                /> <div class="divider">Public</div>
                                <label class="flex gap-2 items-center input input-bordered">
                                    <input
                                        type="text"
                                        class="grow"
                                        placeholder="Artist name"
                                        name="name"
                                        value=artist.get().name
                                    />
                                </label> <DescriptionFields artist=artist.get() />
                                <div class="divider">Private</div>
                                <button class="btn btn-primary">Create</button>
                            </div>
                        </ActionForm>
                    }
                })}
            </ErrorBoundary>
        </Transition>
    };
    var_name
}

/// Managed description so we can preview markdown
#[component]
pub fn DescriptionFields(artist: Artist) -> impl IntoView {
    let (description, set_description) = signal(artist.description);
    let (markdown_description, set_markdown_description) = signal(String::new());
    Effect::new(move || {
        set_markdown_description.set(
            markdown::to_html_with_options(&description.get(), &markdown::Options::gfm()).unwrap(),
        );
    });

    view! {
        <div class="flex gap-6">
            <label class="w-1/2 form-control">
                <h2>Description</h2>
                <textarea
                    class="textarea textarea-bordered"
                    rows="15"
                    name="description"
                    placeholder="Description"
                    prop:value=move || description.get()
                    on:input:target=move |ev| {
                        set_description.set(ev.target().value());
                    }
                >
                    {description}
                </textarea>
                <div class="label">
                    <span class="label-text-alt"></span>
                    <span class="label-text-alt">Markdown</span>
                </div>
            </label>
            <div class="w-1/2">
                <h2>Preview</h2>
                <div inner_html=markdown_description />
            </div>
        </div>
    }
}
