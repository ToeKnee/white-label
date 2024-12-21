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
use reactive_stores::Store;

use crate::components::artist::home::ArtistPage;
use crate::components::record_label::footer::LabelFooter;
use crate::components::record_label::header::LabelHeader;
use crate::components::record_label::home::RecordLabelHome as RecordLabel;
use crate::components::utils::not_found::NotFound;
use crate::state::GlobalState;

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
    // Provide global state context
    provide_context(Store::new(GlobalState::default()));

    view! {
        // injects a stylesheet into the document <head>
        // id=leptos means cargo-leptos will hot-reload this stylesheet
        <Stylesheet id="leptos" href="/pkg/white-label.css" />

        // sets the document title
        <Title text="Welcome to White-Label" />

        <Router>
            <LabelHeader />

            <main class="flex flex-col-reverse gap-6 justify-between my-6 space-y-6 xl:flex-row padding">
                <Routes fallback=NotFound>
                    <Route path=StaticSegment("") view=RecordLabel />
                    <Route path=path!("artists") view=RecordLabel />
                    <Route path=path!("artists/:slug") view=ArtistPage />
                </Routes>
            </main>

            <LabelFooter />
        </Router>
    }
}
