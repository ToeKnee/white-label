//! Track related forms

/// Create a track form
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct CreateTrackForm {
    /// Comma separated list of artist ids
    pub artist_ids: String,
    /// Comma separated list of release ids
    pub release_ids: String,
    /// Name of the track
    pub name: String,
    /// Description of the track
    pub description: String,
    /// Primary artist ID
    pub primary_artist_id: i64,
    /// ISRC code for the track
    pub isrc_code: Option<String>,
    /// BPM (Beats Per Minute) of the track
    pub bpm: Option<i32>,
    /// Published date of the track
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Update a track form
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct UpdateTrackForm {
    /// Comma separated list of artist ids
    pub artist_ids: String,
    /// Comma separated list of release ids
    pub release_ids: String,
    /// Slug of the track
    pub slug: String,
    /// Name of the track
    pub name: String,
    /// Description of the track
    pub description: String,
    /// Primary artist ID
    pub primary_artist_id: i64,
    /// ISRC code for the track
    pub isrc_code: Option<String>,
    /// BPM (Beats Per Minute) of the track
    pub bpm: Option<i32>,
    /// Published date of the track
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}
