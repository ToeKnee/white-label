//! Global store for the application.
#![allow(missing_docs)] // Store macro does not have documentation
use reactive_stores::Store;

use crate::models::{artist::Artist, record_label::RecordLabel};

/// Global state for the application.
#[derive(Clone, Debug, Default, Store)]
pub struct GlobalState {
    /// The currently selected record label.
    ///
    /// `pub(crate)` so the Dioxus UI (`crate::ui`) can read fields through a
    /// `Signal<GlobalState>`; the Leptos side keeps using the generated
    /// `GlobalStateStoreFields` accessors.
    pub(crate) record_label: RecordLabel,
    /// The currently selected artist (same visibility reasoning as above).
    pub(crate) artist: Artist,
}
