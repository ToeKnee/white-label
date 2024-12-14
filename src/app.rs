//! # White Label Application
//!
//! White Label is a web application for managing record labels, artists and releases.
//!
//! This module contains the main application logic and components.
//! If it is in the app, it should be rendering html at some point.

use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    path, StaticSegment,
};

use crate::components::artist::home::ArtistPage;
use crate::components::record_label::header::LabelHeader;
use crate::components::record_label::home::RecordLabelHome as RecordLabel;

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
                    <Route path=StaticSegment("") view=RecordLabel />
                    <Route path=path!("artist/:slug") view=ArtistPage />
                </Routes>
            </main>
        </Router>
    }
}
