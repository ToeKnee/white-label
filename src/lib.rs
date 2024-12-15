//! # White Label
//!
//! White Label is a web application for managing record labels, artists and releases.

//#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::style)]
#![warn(clippy::complexity)]
#![warn(clippy::perf)]

pub mod app;
pub(crate) mod components;
#[cfg(feature = "ssr")]
pub(crate) mod database;
pub(crate) mod models;
pub(crate) mod routes;
#[cfg(feature = "ssr")]
pub mod setup;
pub mod state;
pub mod utils;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
/// Hydrate the application on the client side.
pub fn hydrate() {
    #[allow(unused_imports)]
    use crate::app::WhiteLabelRoot;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(WhiteLabelRoot);
}
