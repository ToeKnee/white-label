//! Global state for the application.
use reactive_stores::Store;

use crate::models::{artist::Artist, label::Label};

/// Global state for the application.
#[derive(Clone, Debug, Default, Store)]
pub struct GlobalState {
    /// The currently selected record label
    record_label: Label,
    /// The list of artists for the currently selected record label
    #[store(key: i64 = |artist| artist.id)]
    artists: Vec<Artist>,
    /// Currently selected artist
    artist: Artist,
}
