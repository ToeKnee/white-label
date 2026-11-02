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

/// Artist index and artist page, plus the shared artist card.
pub mod artist;
/// Dioxus-native data endpoints (`#[get]` server fns) behind the page
/// resources.
pub mod data;
/// Footer with the service/label/legal link columns.
pub mod footer;
/// Shared navbar with the record-label name and the user menu.
pub mod header;
/// Marketing/legal page details.
pub mod page;
/// Page placeholders and the admin layouts referenced by the route table.
///
/// Deleted as Phase 4 slices replace them with real components.
pub mod placeholders;
/// The record label home page.
pub mod record_label;
/// The public release page and its track list.
pub mod release;
/// The application route table and the `redirect` helper.
pub mod routes;
/// The app root: reactive contexts, document head, and the page shell.
pub mod shell;
/// Shared presentational pieces (alerts, badges, spinner, error/404 heroes)
/// and the auth redirect guards — Phase 4 slice 1 of the component port.
pub mod utils;

pub use routes::Route;
pub use routes::redirect;
pub use shell::App;

use dioxus::prelude::*;

use crate::models::auth::User;
use crate::routes::record_label::LabelResult;
use crate::store::GlobalState;

/// The record label load used by the navbar brand and the home/artists
/// headings (created per component — see the resource-scheduling note in
/// `LabelHomeBody`).
///
/// The Phase 0 spike pattern applies: create the
/// resource and `suspend()` on it right away, inside a `SuspenseBoundary`;
/// the boundary then drives the server fn itself.
#[must_use]
pub fn use_label() -> Resource<core::result::Result<LabelResult, HttpError>> {
    use_resource(crate::ui::data::get_record_label)
}

/// Render markdown as HTML with GFM options, the same call the Leptos
/// components made. Broken markdown logs and yields empty HTML rather
/// than falling back silently.
#[must_use]
pub fn markdown_html(source: &str) -> String {
    match markdown::to_html_with_options(source, &markdown::Options::gfm()) {
        Ok(html) => html,
        Err(error) => {
            tracing::warn!("could not render markdown: {error}");
            String::new()
        }
    }
}

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
