//! Response types for the play-statistics pages.
//!
//! The row shapes are the model's (`crate::models::track_play_stats`); this
//! is one page's worth of them: the graph's points and the table's rows.

use crate::models::track_play_stats::{DayTotals, TrackTotals};

/// One stats page's data: the day-by-day points for the graph (oldest
/// first) and the table's rows, in the order the page's `sort` names
/// (complete plays first when its URL carries none).
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct StatsResult {
    /// The scope's counters grouped by day. Days with no counted play are
    /// absent (no point on that day's lines), so a long quiet stretch
    /// costs nothing.
    pub series: Vec<DayTotals>,
    /// The scope's tracks with their counters over the range, zero-play
    /// tracks included.
    pub rows: Vec<TrackTotals>,
}
