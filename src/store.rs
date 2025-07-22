//! Global store for the application.
#![allow(missing_docs)] // Store macro does not have documentation
use reactive_stores::Store;

use crate::models::{artist::Artist, record_label::RecordLabel};

/// Global state for the application.
#[derive(Clone, Debug, Default, Store)]
pub struct GlobalState {
    /// The currently selected record label
    record_label: RecordLabel,
    /// The currently selected artist
    artist: Option<Artist>,
}
