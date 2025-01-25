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
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(WhiteLabel);
}
