//! Global state for the application.
use reactive_stores::Store;

use crate::models::{artist::Artist, label::Label};

/// Global state for the application.
#[derive(Clone, Debug, Default, Store)]
pub struct GlobalState {
    record_label: Label,
    #[store(key: i64 = |artist| artist.id)]
    artists: Vec<Artist>,
}
