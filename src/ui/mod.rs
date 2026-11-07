//! Dioxus UI tree.
//!
//! The whole rendered app: the route table, the shared chrome (navbar,
//! `<main>`, footer), document head elements, the reactive contexts, the
//! pages, forms and auth guards.
//!
//! Only the dioxus targets build it: the SSR server (`server`), the wasm
//! client (`web`) and the desktop window (`desktop`); the mobile target joins
//! in Phase 7. Server-only code inside is gated by the fullstack macros.

/// Admin section (dashboard, label/page editors, `AdminRoot` layout with
/// its side menu and admin gate).
///
/// Phase 4 slice 4; the admin form posts land on `crate::server::forms`
/// handlers under `/admin/forms/*`.
pub mod admin;
/// Artist index and artist page, plus the shared artist card.
pub mod artist;
/// Static file URLs: bundle paths vs files the API server holds.
pub mod assets;
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
/// The artist-tabs layout the artist admin pages render inside.
pub mod placeholders;
/// Shared plumbing for the reactive editors.
pub mod reactive;
/// The record label home page.
pub mod record_label;
/// The public release page and its track list.
pub mod release;
/// The application route table and the `redirect` helper.
pub mod routes;
/// The app root: reactive contexts, document head, and the page shell.
pub mod shell;
/// Uploads with progress: the picker widget and its upload endpoints.
pub mod upload;
/// Shared presentational pieces (alerts, badges, spinner, error/404 heroes)
/// and the auth redirect guards — Phase 4 slice 1 of the component port.
pub mod utils;

pub use routes::Route;
pub use routes::redirect;
pub use shell::App;

use dioxus::prelude::*;

use crate::api_types::record_label::LabelResult;
use crate::models::auth::User;
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

/// The record label load used by the navbar brand, the footer's label column
/// and the home/artists headings (created per component — see the load-order
/// note in `LabelHomeBody`).
///
/// The `?` on the result suspends the enclosing `SuspenseBoundary`.
///
/// # Errors
/// The suspension marker of [`use_page`].
pub fn use_label() -> core::result::Result<PageValue<LabelResult>, RenderError> {
    use_page(move || page_data(crate::ui::data::get_record_label()))
}

/// Render markdown as HTML with GFM options. Broken markdown logs and
/// yields empty HTML rather
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

/// What the last reactive auth action told the shell's chrome.
///
/// The navbar outlives client navigation, and a page resource resolves once
/// per component (see [`use_page`]), so a sign-in that never reloaded the page
/// would otherwise leave the menu showing the session from the original page
/// render. The auth twins in [`crate::ui::auth`] answer with the outcome they
/// just computed, and the menu renders that instead of guessing.
#[derive(Clone, Debug)]
pub enum SessionHint {
    /// Nothing has been signed in or out in this client session: show the
    /// session the server resolved when the page was rendered.
    Unknown,
    /// The account the menu should render, with the whole user (permissions
    /// included) so it needs no refetch. (Boxed: the other variants are bare,
    /// and `User` is far bigger than they are.) Written by a login/register
    /// twin, and by the profile page when a reactive change (an avatar upload)
    /// carries on to the chrome.
    ///
    /// This is what keeps the navbar right on a desktop window: its own
    /// `use_user` page resource resolved once at launch, before anyone had
    /// signed in, so it stays a guest for the life of the client — the menu
    /// rides this hint instead of that stale resource.
    SignedIn(Box<User>),
    /// The logout twin ended the session.
    SignedOut,
}

/// The [`SessionHint`] that [`App`] provides: written by the auth twins,
/// read by the navbar menu.
#[must_use]
pub fn use_session_hint() -> Signal<SessionHint> {
    use_context()
}
