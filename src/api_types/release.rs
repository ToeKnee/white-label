//! Response types for the release endpoints.

use crate::models::{artist::Artist, release::Release, track_with_artists::TrackWithArtists};

/// Contains multiple Releases.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct ReleasesResult {
    /// A vector of releases.
    pub releases: Vec<Release>,
}

/// The result of fetching a single release along with its associated artists and tracks.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct ReleaseResult {
    /// The release being fetched.
    pub release: Release,
    /// A vector of artists associated with the release.
    pub artists: Vec<Artist>,
    /// A vector of tracks with artists associated with the release.
    pub tracks: Vec<TrackWithArtists>,
    /// The release's label guest-play allowance (`None`: the default), so
    /// the public release page can tell the global player how many plays a
    /// guest browser gets per track. Only the release fetch the public page
    /// uses fills it; the editor round trips carry `None`.
    pub guest_plays: Option<i32>,
}
