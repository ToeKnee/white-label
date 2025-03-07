//! # White Label
//!
//! White Label is a web application for managing record labels, artists and releases.

//#![warn(missing_docs)]
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

pub mod app;
pub mod components;
pub mod config;
#[cfg(feature = "ssr")]
pub mod database;
pub mod forms;
pub mod models;
pub mod routes;
#[cfg(feature = "ssr")]
pub mod services;
#[cfg(feature = "ssr")]
pub mod setup;
#[cfg(feature = "ssr")]
pub mod state;
pub mod store;
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
