#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct CreateReleaseForm {
    pub artist_ids: String, // comma separated list of artist ids
    pub name: String,
    pub description: String,
    pub primary_artist_id: i64,
    pub catalogue_number: String,
    pub release_date: Option<chrono::DateTime<chrono::Utc>>,
    pub label_id: i64,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct UpdateReleaseForm {
    pub artist_ids: String, // comma separated list of artist ids
    pub slug: String,
    pub name: String,
    pub description: String,
    pub primary_artist_id: i64,
    pub catalogue_number: String,
    pub release_date: Option<chrono::DateTime<chrono::Utc>>,
    pub label_id: i64,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}
