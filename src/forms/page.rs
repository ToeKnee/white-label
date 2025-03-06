#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct CreatePageForm {
    pub name: String,
    pub description: String,
    pub body: String,
    pub label_id: i64,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct UpdatePageForm {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub body: String,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}
