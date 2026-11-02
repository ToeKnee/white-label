//! Dioxus UI tree (MIGRATION.md, Phase 3: app shell + routing).
//!
//! Where the Dioxus port lives; it replaces the Leptos `app`/`components`
//! stack at cutover. It currently holds the route table, the shared chrome
//! (navbar, `<main>`, footer), document head elements, the reactive
//! contexts, and placeholder pages. The real pages arrive per vertical
//! slice in Phase 4.
//!
//! Only built under the `server` feature for now — the `dioxus` dependency
//! rides that feature so the shipped Leptos build stays untouched. Client
//! targets (web wasm, desktop, mobile) get their own feature gates in
//! Phases 6-7.

/// Footer with the service/label/legal link columns.
pub mod footer;
/// Shared navbar with the record-label name and the user menu.
pub mod header;
/// Page placeholders and the admin layouts referenced by the route table.
///
/// Deleted as Phase 4 slices replace them with real components.
pub mod placeholders;
/// The application route table and the `redirect` helper.
pub mod routes;
/// The app root: reactive contexts, document head, and the page shell.
pub mod shell;

pub use routes::Route;
pub use routes::redirect;
pub use shell::App;

use dioxus::prelude::*;

use crate::models::auth::User;
use crate::store::GlobalState;

/// The [`GlobalState`] store provided by [`App`] and read by the chrome.
///
/// A coarse `Signal` for now; field-level projections if the Phase 4 slices
/// show update patterns that need finer subscriptions.
#[must_use]
pub fn use_global_state() -> Signal<GlobalState> {
    use_context()
}

/// The current user.
///
/// Stays `User::default()` (a guest) until the session check lands with the
/// auth slice in Phase 4, then the header populates it once per load.
#[must_use]
pub fn use_user() -> Signal<User> {
    use_context()
}
