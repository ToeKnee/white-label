//! Track related forms

use super::parse_form_date;

/// Create a track form
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct CreateTrackForm {
    /// Comma separated list of artist ids
    pub artist_ids: String,
    /// Name of the track
    pub name: String,
    /// Description of the track
    pub description: String,
    /// Lyrics of the track
    pub lyrics: String,
    /// Primary artist ID
    pub primary_artist_id: i64,
    /// Release ID
    pub release_id: i64,
    /// ISRC code for the track
    pub isrc_code: Option<String>,
    /// BPM (Beats Per Minute) of the track
    pub bpm: Option<i32>,
    /// Track number on the release
    pub track_number: i32,
    /// Published date of the track
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Update a track form
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct UpdateTrackForm {
    /// Comma separated list of artist ids
    pub artist_ids: String,
    /// Slug of the track
    pub slug: String,
    /// Name of the track
    pub name: String,
    /// Description of the track
    pub description: String,
    /// Lyrics of the track
    pub lyrics: String,
    /// Primary artist ID
    pub primary_artist_id: i64,
    /// Release ID
    pub release_id: i64,
    /// ISRC code for the track
    pub isrc_code: Option<String>,
    /// BPM (Beats Per Minute) of the track
    pub bpm: Option<i32>,
    /// Track number on the release
    pub track_number: i32,
    /// Published date of the track
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// The track create form's posted fields, exactly as the browser posts them.
///
/// `artist_ids` is the "All Artists" checkbox group: repeated once per checked
/// artist without a client build (`crate::server::forms` splits the pairs and
/// joins them), and joined here by the reactive path's form reader.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct TrackCreateFields {
    /// Comma separated list of artist ids (see the struct's docs).
    pub artist_ids: String,
    /// The release the page URL addresses (hidden field and redirect target).
    pub release_id: String,
    /// The artist the page URL addresses (redirect target without a client
    /// build).
    pub artist_slug: String,
    /// The release the page URL addresses (redirect target without a client
    /// build).
    pub release_slug: String,
    /// The track's name; the server derives the slug from it.
    pub name: String,
    /// The track's description; may contain markdown.
    pub description: String,
    /// The track's lyrics; may contain markdown.
    pub lyrics: String,
    /// The primary artist's id, also present in `artist_ids`.
    pub primary_artist_id: String,
    /// The track's ISRC code.
    pub isrc_code: String,
    /// The track's tempo in beats per minute.
    pub bpm: String,
    /// The track's number on the release.
    pub track_number: String,
    /// Hidden full-date field (the stored value).
    pub published_at: String,
    /// Visible `datetime-local` value, preferred when present.
    pub local_published_at: String,
}

impl TrackCreateFields {
    /// The service form these posts turn into, shared by the javascript-off
    /// handler and the reactive twin.
    #[must_use]
    pub fn into_service_form(self) -> CreateTrackForm {
        CreateTrackForm {
            artist_ids: self.artist_ids,
            name: self.name,
            description: self.description,
            lyrics: self.lyrics,
            primary_artist_id: self.primary_artist_id.parse().unwrap_or_default(),
            release_id: self.release_id.parse().unwrap_or_default(),
            // Blank stays Some(""): `tracks.isrc_code` is NOT NULL, and the
            // validate step rejects short codes before insert.
            isrc_code: Some(self.isrc_code),
            bpm: self.bpm.parse().ok(),
            track_number: self.track_number.parse().unwrap_or_default(),
            published_at: parse_form_date(&self.local_published_at, &self.published_at),
        }
    }
}

/// The track update form's posted fields: the create fields plus the addressed
/// slug.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct TrackUpdateFields {
    /// The slug the editor addresses.
    pub slug: String,
    /// Comma separated list of artist ids (see `TrackCreateFields`).
    pub artist_ids: String,
    /// The release the page URL addresses (hidden field and redirect target).
    pub release_id: String,
    /// The artist the page URL addresses (redirect target without a client
    /// build).
    pub artist_slug: String,
    /// The release the page URL addresses (redirect target without a client
    /// build).
    pub release_slug: String,
    /// The track's name; the server re-derives the slug from it.
    pub name: String,
    /// The track's description; may contain markdown.
    pub description: String,
    /// The track's lyrics; may contain markdown.
    pub lyrics: String,
    /// The primary artist's id, also present in `artist_ids`.
    pub primary_artist_id: String,
    /// The track's ISRC code.
    pub isrc_code: String,
    /// The track's tempo in beats per minute.
    pub bpm: String,
    /// The track's number on the release.
    pub track_number: String,
    /// Hidden full-date field (the stored value).
    pub published_at: String,
    /// Visible `datetime-local` value, preferred when present.
    pub local_published_at: String,
}

impl TrackUpdateFields {
    /// The service form these posts turn into.
    #[must_use]
    pub fn into_service_form(self) -> UpdateTrackForm {
        UpdateTrackForm {
            artist_ids: self.artist_ids,
            slug: self.slug,
            name: self.name,
            description: self.description,
            lyrics: self.lyrics,
            primary_artist_id: self.primary_artist_id.parse().unwrap_or_default(),
            release_id: self.release_id.parse().unwrap_or_default(),
            isrc_code: Some(self.isrc_code),
            bpm: self.bpm.parse().ok(),
            track_number: self.track_number.parse().unwrap_or_default(),
            published_at: parse_form_date(&self.local_published_at, &self.published_at),
        }
    }
}

/// Track delete/restore: the track slug plus the artist and release slugs the
/// javascript-off redirect targets are built under.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct TrackSlugFields {
    /// The slug of the track to soft-delete or restore.
    pub slug: String,
    /// The artist the page URL addresses (redirect target without a client
    /// build).
    pub artist_slug: String,
    /// The release the page URL addresses (redirect target without a client
    /// build).
    pub release_slug: String,
}
