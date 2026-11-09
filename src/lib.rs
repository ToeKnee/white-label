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
/// Remembering the login across desktop restarts: which session cookies
/// the OS credential store should hold.
///
/// The web build has no use for it (browsers persist `Max-Age`d cookies
/// themselves).
#[cfg(not(target_arch = "wasm32"))]
pub mod session_store;
/// The app's UI: pages, the hydrated wasm client and the desktop window.
///
/// All three build from this module; server-only parts inside it (the server
/// fns' bodies) are gated by the fullstack macros.
#[cfg(any(feature = "server", feature = "web", feature = "desktop"))]
pub mod ui;
pub mod utils;
