//! Response types for the artist endpoints.

use crate::models::artist::Artist;

/// Contains the result of fetching a single artist.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct ArtistResult {
    /// The artist being fetched.
    pub artist: Artist,
}
