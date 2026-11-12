//! Wire types for the global player's play reports. The counters themselves
//! are the model (`crate::models::track_play_stats`); these are the
//! client's report and the endpoint's answer.

use crate::models::track_play_stats::PlayKind;

/// One finished play the player bar reports: which track (by slug) and how
/// the play ended (see [`crate::ui::player`]'s statistics reporting).
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct PlayReport {
    /// The played track's slug.
    pub track_slug: String,
    /// How the play ended: complete, partial or skip.
    pub kind: PlayKind,
}

/// The report endpoint's answer: whether the play hit the counters.
///
/// A `label_owner` session's plays are deliberately never counted, and that
/// answers `counted: false` rather than an error (the client just heard an
/// owner previewing their own catalogue).
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct PlayCounted {
    /// True when this play was added to the track's counters.
    pub counted: bool,
}
