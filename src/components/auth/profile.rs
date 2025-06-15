//! Profile edit page.
use leptos::form::ActionForm;
use leptos::prelude::*;
use leptos_meta::Title;

use crate::app::UserContext;
use crate::components::files::upload::FileUploadWithProgress;
use crate::components::utils::{
    error::ErrorPage, error::ServerErrors, loading::Loading,
    permissions::authenticated_or_redirect, success::Success,
};
use crate::config::upload::UploadConfiguration;
use crate::models::auth::User;
use crate::routes::auth::UpdateUser;

/// Renders the profile edit page.
#[component]
pub fn EditProfile() -> impl IntoView {
    Effect::new_isomorphic(move || {
        authenticated_or_redirect("/login");
    });

    let user_context = expect_context::<UserContext>();
    // We are using an untracked signal here because we don't want to change this value -
    // it should be the original username.
    let username = RwSignal::new(user_context.0.get_untracked().username);

    let update_user = ServerAction::<UpdateUser>::new();
    let value = Signal::derive(move || {
        update_user
            .value()
            .get()
            .unwrap_or_else(|| Ok(User::default()))
    });
    let (success, set_success) = signal(false);

    view! {
        <Title text="Edit Profile" />
        <article class="my-6 md:container md:mx-auto prose">
            <h1>"Edit Profile"</h1>

            {move || {
                view! {
                    <FileUploadWithProgress
                        config=UploadConfiguration::Avatar
                        slug=username.get()
                    />
                }
            }}

            <Transition fallback=Loading>
                <ErrorBoundary fallback=|_| {
                    ErrorPage
                }>
                    {move || Suspend::new(async move {
                        view! {
                            <ActionForm action=update_user>
                                <div class="grid gap-6">
                                    {move || {
                                        match value.get() {
                                            Ok(fresh_user) => {
                                                if fresh_user.id > 0 {
                                                    user_context.1.set(fresh_user.clone());
                                                    username.set(fresh_user.username);
                                                    if !success.get() {
                                                        set_success.set(true);
                                                    }
                                                } else {
                                                    set_success.set(false);
                                                }

                                                view! { "" }
                                                    .into_any()
                                            }
                                            Err(errors) => {
                                                set_success.set(false);
                                                view! { <ServerErrors server_errors=Some(errors) /> }
                                                    .into_any()
                                            }
                                        }
                                    }}
                                    {move || {
                                        view! {
                                            <Success
                                                message=format!(
                                                    "{} Updated!",
                                                    user_context.0.get().username,
                                                )
                                                show=success.get()
                                            />
                                        }
                                    }}
                                    {move || {
                                        view! {
                                            <Form user=user_context.0.get() username=username />
                                        }
                                    }}
                                </div>
                            </ActionForm>
                        }
                    })}
                </ErrorBoundary>
            </Transition>
        </article>
    }
}

/// The HTML form for editing a user's profile.
#[component]
fn Form(user: User, username: RwSignal<String>) -> impl IntoView {
    view! {
        <input type="text" class="hidden" name="user_form[original_username]" bind:value=username />
        <div class="divider">Public</div>
        <label class="flex gap-2 items-center input">
            <input
                type="text"
                class="grow"
                placeholder="Username"
                name="user_form[username]"
                value=user.username
            />
        </label>
        <div class="divider">Private</div>
        <label class="flex gap-2 items-center input">
            <input
                type="email"
                class="grow"
                placeholder="Email"
                name="user_form[email]"
                value=user.email
            />
        </label>
        <label class="flex gap-2 items-center input">
            <input
                type="text"
                class="grow"
                placeholder="First Name"
                name="user_form[first_name]"
                value=user.first_name
            />
        </label>
        <label class="flex gap-2 items-center input">
            <input
                type="text"
                class="grow"
                placeholder="Last Name"
                name="user_form[last_name]"
                value=user.last_name
            />
        </label>
        <label class="flex gap-2 items-center input">
            <input
                type="text"
                class="grow"
                placeholder="About me - tell us a little about yourself"
                name="user_form[description]"
                value=user.description
            />
        </label>
        <div class="flex flex-auto gap-6">
            <button class="flex-1 btn btn-primary">Update</button>
        </div>
    }
}
