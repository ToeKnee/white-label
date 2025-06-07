//! This module defines the data structures used for creating and updating pages in a web application.

/// Create a new page form structure.
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct CreatePageForm {
    /// Name of the page.
    pub name: String,
    /// Description of the page.
    pub description: String,
    /// Body content of the page. May contain markdown.
    pub body: String,
    /// Record label ID associated with the page.
    pub label_id: i64,
    /// Date and time when the page was published.
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Update an existing page form structure.
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct UpdatePageForm {
    /// Slug of the page, which is a unique identifier.
    pub slug: String,
    /// Name of the page.
    pub name: String,
    /// Description of the page.
    pub description: String,
    /// Body content of the page. May contain markdown.
    pub body: String,
    /// Record label ID associated with the page.
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}
