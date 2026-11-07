//! Release forms and related structures for managing releases in the application.

use super::parse_form_date;

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

/// The release create form's posted fields, exactly as the browser posts them.
///
/// `artist_ids` is the "All Artists" checkbox group: repeated once per checked
/// artist without a client build (`crate::server::forms` splits the pairs and
/// joins them), and joined here by the reactive path's form reader. Every value
/// stays a string because that is what a form posts; `into_service_form` does
/// the parsing.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ReleaseCreateFields {
    /// Comma separated list of artist ids (see the struct's docs).
    pub artist_ids: String,
    /// The label the release is stamped with (a hidden field).
    pub label_id: String,
    /// The artist the page URL addresses (the redirect target without a
    /// client build).
    pub artist_slug: String,
    /// The release's name; the server derives the slug from it.
    pub name: String,
    /// The release's description; may contain markdown.
    pub description: String,
    /// The release's catalogue number.
    pub catalogue_number: String,
    /// The primary artist's id, also present in `artist_ids`.
    pub primary_artist_id: String,
    /// Hidden full-date field (the stored value).
    pub release_date: String,
    /// Hidden full-date field (the stored value).
    pub published_at: String,
    /// Visible `datetime-local` value, preferred when present.
    pub local_release_date: String,
    /// Visible `datetime-local` value, preferred when present.
    pub local_published_at: String,
}

impl ReleaseCreateFields {
    /// The service form these posts turn into, shared by the javascript-off
    /// handler and the reactive twin. An unparsable id parses to 0, which gets
    /// the service's own "no such record" validation rather than inventing a
    /// value.
    #[must_use]
    pub fn into_service_form(self) -> CreateReleaseForm {
        CreateReleaseForm {
            artist_ids: self.artist_ids,
            name: self.name,
            description: self.description,
            primary_artist_id: self.primary_artist_id.parse().unwrap_or_default(),
            catalogue_number: self.catalogue_number,
            release_date: parse_form_date(&self.local_release_date, &self.release_date),
            label_id: self.label_id.parse().unwrap_or_default(),
            published_at: parse_form_date(&self.local_published_at, &self.published_at),
        }
    }
}

/// The release update form's posted fields: the create fields plus the
/// addressed slug.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ReleaseUpdateFields {
    /// The slug the editor addresses.
    pub slug: String,
    /// Comma separated list of artist ids (see `ReleaseCreateFields`).
    pub artist_ids: String,
    /// The label the release is stamped with (a hidden field).
    pub label_id: String,
    /// The artist the page URL addresses (the redirect target without a
    /// client build).
    pub artist_slug: String,
    /// The release's name; the server re-derives the slug from it.
    pub name: String,
    /// The release's description; may contain markdown.
    pub description: String,
    /// The release's catalogue number.
    pub catalogue_number: String,
    /// The primary artist's id, also present in `artist_ids`.
    pub primary_artist_id: String,
    /// Hidden full-date field (the stored value).
    pub release_date: String,
    /// Hidden full-date field (the stored value).
    pub published_at: String,
    /// Visible `datetime-local` value, preferred when present.
    pub local_release_date: String,
    /// Visible `datetime-local` value, preferred when present.
    pub local_published_at: String,
}

impl ReleaseUpdateFields {
    /// The service form these posts turn into.
    #[must_use]
    pub fn into_service_form(self) -> UpdateReleaseForm {
        UpdateReleaseForm {
            artist_ids: self.artist_ids,
            slug: self.slug,
            name: self.name,
            description: self.description,
            primary_artist_id: self.primary_artist_id.parse().unwrap_or_default(),
            catalogue_number: self.catalogue_number,
            release_date: parse_form_date(&self.local_release_date, &self.release_date),
            label_id: self.label_id.parse().unwrap_or_default(),
            published_at: parse_form_date(&self.local_published_at, &self.published_at),
        }
    }
}

/// Release delete/restore: the release slug plus the artist slug the
/// javascript-off redirect targets are built under.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ReleaseSlugFields {
    /// The slug of the release to soft-delete or restore.
    pub slug: String,
    /// The artist the page URL addresses (the redirect target without a
    /// client build).
    pub artist_slug: String,
}
