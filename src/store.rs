//! Global store for the application.
use reactive_stores::Store;

use crate::models::record_label::RecordLabel;

/// Global state for the application.
#[derive(Clone, Debug, Default, Store)]
pub struct GlobalState {
    /// The currently selected record label
    record_label: RecordLabel,
}
