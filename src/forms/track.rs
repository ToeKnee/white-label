#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct CreateTrackForm {
    pub artist_ids: String,  // comma separated list of artist ids
    pub release_ids: String, // comma separated list of release ids
    pub name: String,
    pub description: String,
    pub primary_artist_id: i64,
    pub isrc_code: Option<String>,
    pub bpm: Option<i32>,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct UpdateTrackForm {
    pub artist_ids: String,  // comma separated list of artist ids
    pub release_ids: String, // comma separated list of release ids
    pub slug: String,
    pub name: String,
    pub description: String,
    pub primary_artist_id: i64,
    pub isrc_code: Option<String>,
    pub bpm: Option<i32>,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}
