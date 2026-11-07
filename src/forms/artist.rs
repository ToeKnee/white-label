//! Arist-related form structs.

use super::parse_form_date;

/// The form structs for creating an artist in the admin panel.
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct CreateArtistForm {
    /// The name of the artist.
    pub name: String,
    /// The description of the artist.
    pub description: String,
    /// The artists website URL.
    pub website: String,
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
    /// The artists website URL.
    pub website: String,
    /// The record label ID associated with the artist.
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// The artist create form's posted fields, exactly as the browser posts them.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ArtistCreateFields {
    /// The label the artist is stamped with (a hidden field).
    pub label_id: String,
    /// The artist's name; the server derives the slug from it.
    pub name: String,
    /// The artist's description; may contain markdown.
    pub description: String,
    /// The artist's website URL.
    pub website: String,
    /// Hidden full-date field (the stored value).
    pub published_at: String,
    /// Visible `datetime-local` value, preferred when present.
    pub local_published_at: String,
}

impl ArtistCreateFields {
    /// The service form these posts turn into, shared by the javascript-off
    /// handler and the reactive twin.
    #[must_use]
    pub fn into_service_form(self) -> CreateArtistForm {
        CreateArtistForm {
            name: self.name,
            description: self.description,
            website: self.website,
            label_id: self.label_id.parse().unwrap_or_default(),
            published_at: parse_form_date(&self.local_published_at, &self.published_at),
        }
    }
}

/// The artist update form's posted fields: the create fields plus the
/// addressed slug.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ArtistUpdateFields {
    /// The slug the editor addresses.
    pub slug: String,
    /// The artist's name; the server re-derives the slug from it.
    pub name: String,
    /// The artist's description; may contain markdown.
    pub description: String,
    /// The artist's website URL.
    pub website: String,
    /// Hidden full-date field (the stored value).
    pub published_at: String,
    /// Visible `datetime-local` value, preferred when present.
    pub local_published_at: String,
}

impl ArtistUpdateFields {
    /// The service form these posts turn into.
    #[must_use]
    pub fn into_service_form(self) -> UpdateArtistForm {
        UpdateArtistForm {
            slug: self.slug,
            name: self.name,
            description: self.description,
            website: self.website,
            published_at: parse_form_date(&self.local_published_at, &self.published_at),
        }
    }
}
