//! # White Label Application
//!
//! White Label is a web application for managing record labels, artists and releases.
//!
//! This module contains the main application logic and components.
//! If it is in the app, it should be rendering html at some point.

use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{ParentRoute, Route, Router, Routes},
    path, StaticSegment,
};
use reactive_stores::Store;

use crate::components::{
    admin::{
        artist::create::CreateArtist, artist::edit::EditArtist, dashboard::Dashboard,
        edit_label::EditLabel, root::AdminRoot,
    },
    artist::home::ArtistPage,
    auth::{login::Login, logout::Logout, register::Register},
    record_label::{footer::LabelFooter, header::LabelHeader, home::RecordLabelHome},
    utils::{error::ErrorPage, loading::Loading, not_found::NotFound},
};
use crate::models::auth::User;
use crate::routes::auth::get_user;
use crate::store::GlobalState;

/// HTML shell for the application.
#[must_use]
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <AutoReload options=options.clone() />
                <HydrationScripts options />
                <MetaTags />
            </head>
            <body>
                <WhiteLabel />
            </body>
        </html>
    }
}

#[derive(Copy, Clone)]
pub struct UserContext(pub ReadSignal<User>, pub WriteSignal<User>);

/// Renders the main application.
#[component]
#[must_use]
pub fn WhiteLabel() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();
    // Provide global state context
    provide_context(Store::new(GlobalState::default()));

    let (user, set_user) = signal(User::default());
    let user_resource = Resource::new(move || (user.get()), move |_| get_user());
    provide_context(UserContext(user, set_user));

    view! {
        // injects a stylesheet into the document <head>
        // id=leptos means cargo-leptos will hot-reload this stylesheet
        <Stylesheet id="leptos" href="/pkg/white-label.css" />

        // sets the document title
        <Title text="Welcome to White-Label" />

        // This transition is used for loading the user and storing it in context.
        <Transition fallback=Loading>
            <ErrorBoundary fallback=|_| {
                ErrorPage
            }>
                {move || Suspend::new(async move {
                    (user_resource.await)
                        .map_or_else(
                            |_| Some(User::default()),
                            |this_user| {
                                *set_user.write() = this_user.clone().unwrap_or_default();
                                this_user
                            },
                        );
                })}
            </ErrorBoundary>
        </Transition>

        <Router>
            <LabelHeader />

            <main class="flex flex-col-reverse gap-6 justify-between my-6 space-y-6 xl:flex-row padding">
                <Routes fallback=NotFound>
                    <Route path=StaticSegment("") view=RecordLabelHome />
                    <Route path=path!("artists") view=RecordLabelHome />
                    <Route path=path!("artists/:slug") view=ArtistPage />

                    <Route path=path!("login") view=Login />
                    <Route path=path!("register") view=Register />
                    <Route path=path!("logout") view=Logout />

                    <ParentRoute path=path!("admin") view=AdminRoot>
                        <Route path=path!("") view=Dashboard />
                        <Route path=path!("label") view=EditLabel />
                        <Route path=path!("artist") view=CreateArtist />
                        <Route path=path!("artist/:slug") view=EditArtist />
                    </ParentRoute>
                </Routes>
            </main>

            <LabelFooter />

        </Router>
    }
}
