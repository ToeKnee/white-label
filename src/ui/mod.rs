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

/// Admin section (dashboard, label/page editors, `AdminRoot` layout with
/// its side menu and admin gate).
///
/// Phase 4 slice 4; the admin form posts land on `crate::server::forms`
/// handlers under `/admin/forms/*`.
pub mod admin;
/// Artist index and artist page, plus the shared artist card.
pub mod artist;
/// Auth pages (login/register/logout/profile/change-password).
///
/// Phase 4 slice 3; form posts are handled by `crate::server::forms`.
pub mod auth;
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
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// The error a page resource carries: the message and status the server
/// fn answered with, kept as plain data.
///
/// [`HttpError`] is not serde-serializable, and dioxus' hydration transport
/// requires the whole resource value to be — so page loads carry this copy
/// of the two fields that matter to the UI.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PageFailure {
    /// The message the server fn sent, already phrased for readers.
    pub message: String,
    /// The HTTP status the server fn answered with.
    pub status: u16,
}

impl std::fmt::Display for PageFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(formatter)
    }
}

impl std::error::Error for PageFailure {}

/// Result shape carried by the page resources (see [`page_data`]): the
/// loaded value, or the error the server fn answered with.
pub type PageLoad<T> = core::result::Result<T, PageFailure>;

/// Wraps a page data fn so [`use_page`] can carry its result through
/// hydration.
///
/// The value dioxus transports to the client must be serde-serializable,
/// and [`HttpError`] is not, so it becomes a [`PageFailure`] here — pages
/// log the message and fall back to defaults, while the real status still
/// reaches callers of the JSON twins.
///
/// ```ignore
/// let artist = use_page(move || page_data(get_artist(slug())))?;
/// ```
///
/// # Errors
/// The [`PageFailure`] the server fn answered with (status and message);
/// the load itself never fails on the client side beyond that.
pub async fn page_data<T>(
    load: impl std::future::Future<Output = core::result::Result<T, HttpError>> + 'static,
) -> PageLoad<T> {
    load.await.map_err(|error| PageFailure {
        status: error.status.as_u16(),
        message: error
            .message
            .unwrap_or_else(|| format!("request failed with status {}", error.status.as_u16())),
    })
}

/// The read handle [`use_page`] hands back once the boundary has
/// suspended: calling it (`x()`) gives the loaded [`PageLoad`].
pub type PageValue<T> = MappedSignal<PageLoad<T>, Signal<Option<PageLoad<T>>>>;

/// Starts a page data load, suspending the enclosing `SuspenseBoundary`
/// until the answer is in hand.
///
/// Built on [`use_server_future`] rather than `use_resource`: it is the
/// only dioxus hook that hands the client the result the server already
/// resolved. Without that, the client refetches during its first render and
/// shows suspense fallbacks where the server rendered content, so hydration
/// stops matching node for node and the interpreter dies on an unknown node
/// id. Inside the closure, [`page_data`] swaps the page fn's [`HttpError`]
/// for [`PageFailure`], since the value has to survive serde to reach the
/// client.
///
/// ```ignore
/// let artist = use_page(move || page_data(get_artist(slug())))?;
/// match artist() {
///     Ok(result) => rsx! { "{result.artist.name}" },
///     Err(error) => rsx! { "Could not load the artist: {error}" },
/// }
/// ```
///
/// # Errors
/// The suspension marker dioxus uses to hand the load to the enclosing
/// `SuspenseBoundary`; a component inside one propagates it with `?`, which
/// is what callers do. A load that *fails* arrives as `Ok` with a
/// [`PageLoad`] error value.
pub fn use_page<T, F>(
    load: impl FnMut() -> F + 'static,
) -> core::result::Result<PageValue<T>, RenderError>
where
    T: Clone + Serialize + DeserializeOwned + 'static,
    F: std::future::Future<Output = PageLoad<T>> + 'static,
{
    // `use_server_future` has already suspended the boundary while the
    // value was pending, so `suspend()` here returns the ready handle.
    let resource = use_server_future(load)?;
    resource.suspend()
}

/// The record label load used by the navbar brand and the home/artists
/// headings (created per component — see the load-order note in
/// `LabelHomeBody`).
///
/// The `?` on the result suspends the enclosing `SuspenseBoundary`.
///
/// # Errors
/// The suspension marker of [`use_page`].
pub fn use_label() -> core::result::Result<PageValue<LabelResult>, RenderError> {
    use_page(move || page_data(crate::ui::data::get_record_label()))
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

/// The current user's session, loaded through the `GET /api/user` twin.
///
/// Like [`use_label`], it suspends through the enclosing `SuspenseBoundary`;
/// reading the resource gives a signed-in session as `Some(User)`, and
/// guests and load failures as `None`. Guards and the navbar build their
/// menus from this.
///
/// # Errors
/// The suspension marker of [`use_page`].
pub fn use_user() -> core::result::Result<PageValue<Option<User>>, RenderError> {
    use_page(move || page_data(crate::ui::data::get_current_user()))
}
