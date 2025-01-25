#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct CreateArtistForm {
    pub name: String,
    pub description: String,
    pub label_id: i64,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct UpdateArtistForm {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}
