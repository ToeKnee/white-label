//! Response types for the track endpoints.

use crate::models::{artist::Artist, track::Track, track_with_artists::TrackWithArtists};

/// The result of fetching a single track along with its associated artists and releases.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct TrackResult {
    /// The track being fetched.
    pub track: Track,
    /// A vector of artists associated with the track.
    pub artists: Vec<Artist>,
}

/// One track the global player can queue next, with the name of the release
/// it comes from (what the player bar shows as the track's second line when
/// the track has no featured artists).
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct UpcomingAudio {
    /// The track and its artists.
    pub track: TrackWithArtists,
    /// The release the track sits on.
    pub release_name: String,
    /// The release's image column (stored file name, `None` when the release
    /// has none): the artwork the player bar falls back to when the track has
    /// no image of its own.
    pub release_image: Option<String>,
}

/// A batch of tracks to top the global player's queue up with (see
/// `crate::ui::player`'s auto-append): always tracks with audio, never one
/// the queue already holds.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct ContinuationResult {
    /// The tracks to append, in the order they should play.
    pub tracks: Vec<UpcomingAudio>,
}
