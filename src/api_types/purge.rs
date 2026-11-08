//! Types for the admin "housekeeping" purge.

/// How long a soft-deleted row stays restorable before the purge may
/// remove it permanently.
///
/// Shared between the service (the cutoff) and the dashboard card (the
/// message), so the UI can never promise a different window than the
/// database enforces.
pub const PURGE_AFTER_DAYS: i64 = 90;

/// What a purge removed — or, for a preview, what a purge would remove.
///
/// One number per table:
///
/// ```text
/// POST /api/purge_deleted  {"confirm":false}
/// 200 {"Saved":{"tracks":4,"releases":2,"artists":1,"pages":0,"skipped_artists":3}}
/// ```
///
/// `skipped_artists` counts the past-window artists that were *not*
/// removed: a surviving row still names one as its primary artist, and
/// removing it would leave that row dangling (see
/// [`crate::services::purge`] for the rules).
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct PurgeResult {
    /// Tracks permanently deleted (their own past-window deletions, plus
    /// tracks under purged releases).
    pub tracks: u64,
    /// Releases permanently deleted.
    pub releases: u64,
    /// Artists permanently deleted.
    pub artists: u64,
    /// Pages permanently deleted.
    pub pages: u64,
    /// Past-window artists left alone because a surviving row still names
    /// them as its primary artist.
    pub skipped_artists: u64,
}

impl PurgeResult {
    /// Total rows removed (or previewed): the per-table counts added up.
    #[must_use]
    pub const fn total(&self) -> u64 {
        self.tracks + self.releases + self.artists + self.pages
    }
}
