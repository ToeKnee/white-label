use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::app::UserContext;
use crate::components::utils::{
    error::ErrorPage, error::ServerErrors, loading::Loading, success::Success,
};
use crate::models::artist::Artist;
use crate::routes::artist::{get_artist, ArtistResult, UpdateArtist};
use crate::utils::redirect::redirect;

/// Renders the create artist page.
#[component]
pub fn EditArtist() -> impl IntoView {
    let params = use_params_map();
    let slug = params.read().get("slug").unwrap();
    let slug = RwSignal::new(slug);

    let user_context = expect_context::<UserContext>();
    let (user, _set_user) = signal(user_context.0.get());

    Effect::new_isomorphic(move || {
        if user.get().is_active() && !user.get().permissions.contains("label_owner") {
            redirect("/");
        }
    });

    let (artist, set_artist) = signal(Artist::default());
    let artist_resource = Resource::new(move || slug, |slug| get_artist(slug.get()));

    let update_artist = ServerAction::<UpdateArtist>::new();
    let value = Signal::derive(move || {
        update_artist
            .value()
            .get()
            .unwrap_or_else(|| Ok(ArtistResult::default()))
    });
    let (success, set_success) = signal(false);

    let var_name = view! {
        <h1>Create Artist</h1>

        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    if let Ok(this_artist) = artist_resource.await {
                        set_artist.set(this_artist.artist);
                    } else {
                        redirect("/admin/artists");
                    };
                    view! {
                        <ActionForm action=update_artist>
                            <div class="grid gap-6">
                                {move || {
                                    match value.get() {
                                        Ok(artist_result) => {
                                            let fresh_artist = artist_result.artist;
                                            if fresh_artist.id > 0 {
                                                if fresh_artist.slug != artist.get().slug {
                                                    set_artist.set(fresh_artist.clone());
                                                    slug.set(fresh_artist.clone().slug);
                                                    redirect(&format!("/admin/artist/{}", fresh_artist.slug));
                                                }
                                                if !success.get() {
                                                    set_artist.set(fresh_artist);
                                                    set_success.set(true);
                                                }
                                            } else {
                                                set_success.set(false);
                                            }
                                            view! { "" }.into_any()
                                        }
                                        Err(errors) => {
                                            set_success.set(false);
                                            view! { <ServerErrors server_errors=Some(errors) /> }
                                                .into_any()
                                        }
                                    }
                                }}
                                {move || {
                                    if success.get() {
                                        view! {
                                            <Success message=format!(
                                                "{} Updated!",
                                                artist.get().name,
                                            ) />
                                        }
                                            .into_any()
                                    } else {
                                        view! { "" }.into_any()
                                    }
                                }} <input type="text" class="hidden" name="slug" bind:value=slug />
                                <div class="divider">Public</div>
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
                                <button class="btn btn-primary">Update</button>
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
