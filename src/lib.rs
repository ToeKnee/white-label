//! # White Label
//!
//! White Label is a web application for managing record labels, artists and releases.

#![recursion_limit = "256"]
// Deeply nested `view!` trees (the ErrorBoundary in app.rs wrapping
// components/artist/home.rs) overflow rustc 1.99's default recursion budget
// when compiling the lib-test target; this is rustc's own suggested remedy.
#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::complexity)]
#![warn(clippy::nursery)]
#![warn(clippy::perf)]
#![warn(clippy::style)]
// This pattern works well for components/routes etc.
#![allow(clippy::module_name_repetitions)]
// False positive on `must_use` for components
#![allow(clippy::must_use_candidate)]

/// Wire response types shared by the services, the endpoints and the UI
/// (framework-free, so the Dioxus client builds can use them).
pub mod api_types;
// The Leptos UI (`app`, `components`) and its endpoints (`routes`) are only
// built for the Leptos bundles: SSR and `hydrate`. The Dioxus UI (`ui`) is
// the build used by `web` and `desktop`, so leaving these out keeps the wasm
// bundle and the native binary from carrying dead Leptos components.
#[cfg(any(feature = "ssr", feature = "hydrate"))]
pub mod app;
#[cfg(any(feature = "ssr", feature = "hydrate"))]
pub mod components;
pub mod config;
#[cfg(feature = "ssr")]
pub mod database;
pub mod error;
pub mod forms;
pub mod models;
#[cfg(any(feature = "ssr", feature = "hydrate"))]
pub mod routes;
#[cfg(feature = "server")]
pub mod server;
#[cfg(feature = "ssr")]
pub mod services;
#[cfg(feature = "ssr")]
pub mod setup;
#[cfg(feature = "ssr")]
pub mod state;
pub mod store;
/// Dioxus UI port (MIGRATION.md Phase 3+).
///
/// Compiles for the SSR server (`server`), the wasm client (`web`) and the
/// webview-window client (`desktop`); server-only code inside it (the server
/// fns' bodies) is gated by the fullstack macros.
#[cfg(any(feature = "server", feature = "web", feature = "desktop"))]
pub mod ui;
pub mod utils;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
/// Hydrate the application on the client side.
pub fn hydrate() {
    #[allow(unused_imports)]
    use crate::app::WhiteLabel;

    // print pretty errors in wasm https://github.com/rustwasm/console_error_panic_hook
    // This is not needed for tracing_wasm to work, but it is a common tool for getting proper error line numbers for panics.
    console_error_panic_hook::set_once();

    #[cfg(debug_assertions)]
    let level = tracing::Level::TRACE;
    #[cfg(not(debug_assertions))]
    let level = tracing::Level::ERROR;

    let config = tracing_wasm::WASMLayerConfigBuilder::new()
        .set_max_level(level)
        .set_report_logs_in_timings(false)
        .build();
    tracing_wasm::set_as_global_default_with_config(config);

    leptos::mount::hydrate_body(WhiteLabel);
}
