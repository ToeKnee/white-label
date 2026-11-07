//! Response types for the track endpoints.

use crate::models::{artist::Artist, track::Track};

/// The result of fetching a single track along with its associated artists and releases.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct TrackResult {
    /// The track being fetched.
    pub track: Track,
    /// A vector of artists associated with the track.
    pub artists: Vec<Artist>,
}
