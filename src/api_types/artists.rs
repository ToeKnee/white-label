//! Response types for the multi-artist endpoints.

use crate::models::release::Release;

/// Contains multiple releases for an artist.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct ArtistsReleaseResult {
    /// A vector of releases associated with the specified artist IDs.
    pub releases: Vec<Release>,
}
