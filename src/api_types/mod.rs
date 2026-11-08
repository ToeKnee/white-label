//! Wire types shared between the service layer and whatever talks to it.
//!
//! These are the response half of the API surface (`crate::forms` holds the
//! request half). They are deliberately free of any framework type so they
//! compile in every build: the SSR server, the wasm Dioxus client (`web`)
//! and the webview Dioxus client (`desktop`).
//!
//! Services return them, the `crate::server` endpoints and the `#[get]`
//! server fns in `crate::ui::data` serialize them, and `crate::ui`
//! consumes them as plain data:
//!
//! ```
//! use white_label::api_types::artist::ArtistResult;
//! use white_label::models::artist::Artist;
//!
//! let result = ArtistResult {
//!     artist: Artist::default(),
//! };
//! assert!(result.artist.slug.is_empty());
//! ```

pub mod artist;
pub mod auth;
pub mod edit;
pub mod links;
pub mod menu;
pub mod page;
pub mod purge;
pub mod record_label;
pub mod release;
pub mod track;
