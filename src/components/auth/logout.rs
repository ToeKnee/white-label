use crate::routes::auth::Logout;
use leptos::form::ActionForm;
use leptos::logging;
use leptos::prelude::*;

/// Renders the login page.
#[component]
pub fn Logout() -> impl IntoView {
    let logout = ServerAction::<Logout>::new();
    // holds the latest *returned* value from the server
    let value = logout.value();
    // check if the server has returned an error
    //let has_error = move || login.with(|val| matches!(val, Some(Err(_))));
    logging::log!("value {:#?}", value);
    //logging::log!("has_error {}", has_error());

    view! {
        <article class="md:container md:mx-auto prose">
            <h1>Log out</h1>
            <ActionForm action=logout>
                <div class="grid gap-6">

                    <button class="btn btn-primary">Log out</button>
                </div>
            </ActionForm>
        </article>
    }
}
