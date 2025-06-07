//! Release forms and related structures for managing releases in the application.

/// Create a release form
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct CreateReleaseForm {
    /// Comma separated list of artist ids
    pub artist_ids: String,
    /// Name of the release
    pub name: String,
    /// Description of the release
    pub description: String,
    /// Primary artist ID, this should also be in the `artist_ids` list
    pub primary_artist_id: i64,
    /// Catalogue number for the release
    pub catalogue_number: String,
    /// Release date of the release
    pub release_date: Option<chrono::DateTime<chrono::Utc>>,
    /// Record label ID associated with the release
    pub label_id: i64,
    /// Published date of the release
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Update a release form
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct UpdateReleaseForm {
    /// Comma separated list of artist ids
    pub artist_ids: String,
    /// Slug of the release
    pub slug: String,
    /// Name of the release
    pub name: String,
    /// Description of the release
    pub description: String,
    /// Primary artist ID, this should also be in the `artist_ids` list
    pub primary_artist_id: i64,
    /// Catalogue number for the release
    pub catalogue_number: String,
    /// Release date of the release
    pub release_date: Option<chrono::DateTime<chrono::Utc>>,
    /// Record label ID associated with the release
    pub label_id: i64,
    /// Published date of the release
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}
