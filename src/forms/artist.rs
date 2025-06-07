//! Arist-related form structs.

/// The form structs for creating an artist in the admin panel.
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct CreateArtistForm {
    /// The name of the artist.
    pub name: String,
    /// The description of the artist.
    pub description: String,
    /// The record label ID associated with the artist.
    pub label_id: i64,
    /// The date and time when the artist was published.
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// The form structs for updating an artist in the admin panel.
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct UpdateArtistForm {
    /// The slug of the artist.
    pub slug: String,
    /// The name of the artist.
    pub name: String,
    /// The description of the artist.
    pub description: String,
    /// The record label ID associated with the artist.
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}
