//! # White Label Application
//!
//! White Label is a web application for managing record labels, artists and releases.
//!
//! This module contains the main application logic and components.
//! If it is in the app, it should be rendering html at some point.

use leptos::prelude::*;
use leptos_meta::{MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::{
    StaticSegment,
    components::{ParentRoute, Route, Router, Routes},
    path,
};
use reactive_stores::Store;

use crate::components::{
    admin::{
        artist::{
            create::CreateArtist,
            edit::EditArtist,
            images::EditArtistImages,
            links::EditArtistLinks,
            release::{
                create::CreateRelease,
                edit::EditRelease,
                list::Releases,
                track::{create::CreateTrack, edit::EditTrack, list::Tracks},
            },
            root::AdminArtistRoot,
        },
        dashboard::Dashboard,
        edit_label::EditLabel,
        page::create::CreatePage,
        page::edit::EditPage,
        root::AdminRoot,
    },
    artist::{home::ArtistPage, list::ArtistsPage, release::ReleasePage},
    auth::{
        change_password::ChangePassword, login::Login, logout::Logout, profile::EditProfile,
        register::Register,
    },
    page::PageDetails,
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

/// Context for managing user state across the application.
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

    let formatter = move |text: String| text;

    view! {
        // injects a stylesheet into the document <head>
        // id=leptos means cargo-leptos will hot-reload this stylesheet
        <Stylesheet id="leptos" href="/pkg/white-label.css" />

        // sets the document title
        <Title formatter />

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
                                if this_user.clone().unwrap_or_default().is_authenticated()
                                    && !user.get().is_authenticated()
                                {
                                    if let Some(ref user) = this_user {
                                        *set_user.write() = user.clone();
                                    }
                                }
                                this_user
                            },
                        );
                })}
            </ErrorBoundary>
        </Transition>

        <Router>
            <LabelHeader />

            <main>
                <Routes fallback=NotFound>
                    <Route path=StaticSegment("") view=RecordLabelHome />
                    <Route path=path!("artists") view=ArtistsPage />
                    <Route path=path!("artists/") view=ArtistsPage />
                    <Route path=path!("artists/:slug") view=ArtistPage />
                    <Route path=path!("artists/:slug/") view=ArtistPage />
                    <Route path=path!("artists/:artist_slug/:release_slug") view=ReleasePage />
                    <Route path=path!("pages/:slug") view=PageDetails />

                    <Route path=path!("login") view=Login />
                    <Route path=path!("register") view=Register />
                    <Route path=path!("logout") view=Logout />
                    <Route path=path!("profile") view=EditProfile />
                    <Route path=path!("profile/change-password") view=ChangePassword />

                    <ParentRoute path=path!("admin") view=AdminRoot>
                        <Route path=path!("") view=Dashboard />
                        <Route path=path!("label") view=EditLabel />
                        <ParentRoute path=path!("artist") view=AdminArtistRoot>
                            <Route path=path!("") view=CreateArtist />
                            <Route path=path!(":slug") view=EditArtist />
                            <Route path=path!(":slug/") view=EditArtist />
                            <Route path=path!(":slug/links") view=EditArtistLinks />
                            <Route path=path!(":slug/links/") view=EditArtistLinks />
                            <Route path=path!(":slug/images") view=EditArtistImages />
                            <Route path=path!(":slug/images/") view=EditArtistImages />
                            <Route path=path!(":slug/releases") view=Releases />
                            <Route path=path!(":slug/releases/") view=Releases />
                            <Route path=path!(":slug/releases/new") view=CreateRelease />
                            <Route path=path!(":slug/release/:release_slug") view=EditRelease />
                            <Route path=path!(":slug/release/:release_slug/") view=EditRelease />
                            <Route path=path!(":slug/release/:release_slug/tracks") view=Tracks />
                            <Route path=path!(":slug/release/:release_slug/tracks/") view=Tracks />
                            <Route
                                path=path!(":slug/release/:release_slug/tracks/new")
                                view=CreateTrack
                            />
                            <Route
                                path=path!(":slug/release/:release_slug/track/:track_slug")
                                view=EditTrack
                            />
                        </ParentRoute>
                        <Route path=path!("page") view=CreatePage />
                        <Route path=path!("page/") view=CreatePage />
                        <Route path=path!("page/:slug") view=EditPage />
                    </ParentRoute>
                </Routes>
            </main>

            <LabelFooter />

        </Router>
    }
}
