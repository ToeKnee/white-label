//! This module defines a structure for a track along with its associated artists.

use serde::{Deserialize, Serialize};

use super::{artist::Artist, track::Track};

/// Represents a track along with its associated artists.
#[derive(Serialize, Deserialize, Clone, Default, Debug, Eq, PartialEq)]
pub struct TrackWithArtists {
    /// The track
    pub track: Track,
    /// A vector of artists associated with the track
    pub artists: Vec<Artist>,
}
