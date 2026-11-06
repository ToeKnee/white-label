//! # White Label
//!
//! White Label is a web application for managing record labels, artists and releases.

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

/// Wire response types shared by the services and the API (framework-free, so
/// the Dioxus client builds can use them).
pub mod api_types;
pub mod config;
#[cfg(feature = "server")]
pub mod database;
pub mod error;
pub mod forms;
pub mod models;
#[cfg(feature = "server")]
pub mod server;
#[cfg(feature = "server")]
pub mod services;
/// The app's UI: pages, the hydrated wasm client and the desktop window.
///
/// All three build from this module; server-only parts inside it (the server
/// fns' bodies) are gated by the fullstack macros.
#[cfg(any(feature = "server", feature = "web", feature = "desktop"))]
pub mod ui;
pub mod utils;
