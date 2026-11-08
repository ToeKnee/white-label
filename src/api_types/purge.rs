//! Types for the admin "housekeeping" purge.

/// How long a soft-deleted row stays restorable before the purge may
/// remove it permanently.
///
/// Shared between the service (the cutoff) and the dashboard card (the
/// message), so the UI can never promise a different window than the
/// database enforces.
pub const PURGE_AFTER_DAYS: i64 = 90;

/// The rows a purge removed — or, for a preview, the rows it would
/// remove — table by table, named.
///
/// The preview names the rows before anything is deleted, so the card
/// can show exactly what is about to go:
///
/// ```text
/// POST /api/purge_deleted  {"confirm":false}
/// 200 {"Saved":{"tracks":["Test Track 1"],
///      "releases":[],"artists":[],"pages":["Test Page 1"],
///      "skipped_artists":["Test Artist 2"]}}
/// ```
///
/// `skipped_artists` names the past-window artists that were *not*
/// removed: a surviving row still names one as its primary artist, and
/// removing it would leave that row dangling (see
/// [`crate::services::purge`] for the rules).
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct PurgeResult {
    /// Tracks permanently deleted (their own past-window deletions, plus
    /// tracks under purged releases).
    pub tracks: Vec<String>,
    /// Releases permanently deleted.
    pub releases: Vec<String>,
    /// Artists permanently deleted.
    pub artists: Vec<String>,
    /// Pages permanently deleted.
    pub pages: Vec<String>,
    /// Past-window artists left alone because a surviving row still names
    /// them as their primary artist.
    pub skipped_artists: Vec<String>,
}

impl PurgeResult {
    /// Total rows removed (or previewed): the per-table lists added up.
    #[must_use]
    pub const fn total(&self) -> usize {
        self.tracks.len() + self.releases.len() + self.artists.len() + self.pages.len()
    }
}
