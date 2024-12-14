//! # White Label
//!
//! White Label is a web application for managing record labels, artists and releases.

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::style)]
#![warn(clippy::complexity)]
#![warn(clippy::perf)]

/// Main function for the server-side rendered application.
#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    white_label::setup::init_app(None).await;
}

/// There is no client-side main function - we are using SSR and hydrating the app instead.
#[cfg(not(feature = "ssr"))]
pub fn main() {
    // no client-side main function
    // unless we want this to work with e.g., Trunk for pure client-side testing
    // see lib.rs for hydration function instead
}
