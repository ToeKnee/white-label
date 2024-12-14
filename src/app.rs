//! # White Label Application
//!
//! White Label is a web application for managing record labels, artists and releases.
//!
//! This module contains the main application logic and components.
//! If it is in the app, it should be rendering html at some point.

use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes, A},
    hooks::use_params_map,
    path, StaticSegment,
};
use markdown;

use crate::routes::artist::get_artist;
use crate::routes::label::get_label;

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
                <WhiteLabelRoot />
            </body>
        </html>
    }
}

/// Renders the main application.
#[component]
#[must_use]
pub fn WhiteLabelRoot() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    view! {
        // injects a stylesheet into the document <head>
        // id=leptos means cargo-leptos will hot-reload this stylesheet
        <Stylesheet id="leptos" href="/pkg/white-label.css" />

        // sets the document title
        <Title text="Welcome to White-Label" />

        <LabelHeader />
        // content for this welcome page
        <Router>
            <main>
                <Routes fallback=|| "Page not found.".into_view()>
                    <Route path=StaticSegment("") view=HomePage />
                    <Route path=path!("artist/:slug") view=ArtistPage />
                </Routes>
            </main>
        </Router>
    }
}

/// Renders the home page of your application.
#[component]
fn HomePage() -> impl IntoView {
    view! { <RecordLabel /> }
}

/// Renders the record label page.
//#[tracing::instrument]
#[component]
pub fn LabelHeader() -> impl IntoView {
    view! {
        <Transition fallback=move || view! { <p>"Loading Record Label"</p> }>
            <ErrorBoundary fallback=|_| {
                view! { <p class="error-messages text-xs-center">"Something went wrong."</p> }
            }>
                {move || Suspend::new(async move {
                    let resource = get_label().await;
                    let label = resource.unwrap().label;
                    view! { <h1 class="text-4xl font-bold">{label.name}</h1> }
                })}

            </ErrorBoundary>
        </Transition>
    }
}

/// Renders the record label page.
//#[tracing::instrument]
#[component]
pub fn RecordLabel() -> impl IntoView {
    view! {
        <Transition fallback=move || view! { <p>"Loading Record Label"</p> }>
            <ErrorBoundary fallback=|_| {
                view! { <p class="error-messages text-xs-center">"Something went wrong."</p> }
            }>
                {move || Suspend::new(async move {
                    let resource = get_label().await;
                    let label = resource.unwrap().label;
                    view! {
                        <h2>{label.name}</h2>
                        <div inner_html=markdown::to_html(&label.description) />
                        <A href=format!("/artist/janky-switch")>"View Artists"</A>
                    }
                })}

            </ErrorBoundary>
        </Transition>
    }
}

/// Renders the record label page.
//#[tracing::instrument]
#[component]
pub fn ArtistPage() -> impl IntoView {
    let params = use_params_map();

    let artist_res = Resource::new(
        move || params.get(),
        |slug| async move {
            if let Some(s) = slug.get("slug") {
                get_artist(s.to_string()).await
            } else {
                get_artist("janky-switch".to_string()).await
            }
        },
    );

    view! {
        <Transition fallback=move || view! { <p>"Loading Artist"</p> }>
            <ErrorBoundary fallback=|_| {
                view! { <p class="error-messages text-xs-center">"Something went wrong."</p> }
            }>
                {move || {
                    artist_res
                        .get()
                        .map(move |x| {
                            x.map(move |a| {
                                view! {
                                    <h1>{a.artist.name}</h1>
                                    <div inner_html=markdown::to_html(&a.artist.description) />
                                }
                            })
                        })
                }}
            </ErrorBoundary>
        </Transition>
    }
}
