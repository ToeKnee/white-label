//! Per-track play counters.
//!
//! The global player reports each play it stops (see [`crate::ui::player`]'s
//! reporting), and [`crate::services::track_play_stats`] decides which
//! reports count before one of the three columns here is incremented.

use serde::{Deserialize, Serialize};
#[cfg(feature = "server")]
use sqlx::{FromRow, PgPool};

/// How one reported play ended. The marks come from the Ideas list: complete
/// past 90% of the track, partial past 10% but not 90%, skip before 10%.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlayKind {
    /// Played past the 90% mark — including a track that ended by itself.
    Complete,
    /// Played past the 10% mark, then stopped before 90%.
    Partial,
    /// Stopped before the 10% mark.
    Skip,
}

impl PlayKind {
    /// Which counter this kind bumps, as the `(complete, partial, skip)`
    /// triple the upsert adds: the kind's own column gets 1, the others 0.
    #[cfg(feature = "server")]
    const fn counters(self) -> (i32, i32, i32) {
        match self {
            Self::Complete => (1, 0, 0),
            Self::Partial => (0, 1, 0),
            Self::Skip => (0, 0, 1),
        }
    }
}

/// The stored counters for one track. `updated_at` is deliberately not a
/// field: the upsert maintains the column, and no read shows it.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(FromRow))]
pub struct TrackPlayStats {
    /// The track these counters belong to.
    pub track_id: i64,
    /// Plays that passed the 90% mark.
    pub complete_plays: i32,
    /// Plays that passed 10% but stopped before 90%.
    pub partial_plays: i32,
    /// Plays stopped before the 10% mark.
    pub skips: i32,
}

impl TrackPlayStats {
    /// The counters stored for `track_id`, or `None` when no counted play
    /// has landed for it yet.
    ///
    /// # Errors
    /// If the database query fails, return an error
    #[cfg(feature = "server")]
    pub async fn get_by_track_id(pool: &PgPool, track_id: i64) -> anyhow::Result<Option<Self>> {
        sqlx::query_as::<_, Self>(
            "SELECT track_id, complete_plays, partial_plays, skips \
             FROM track_play_stats WHERE track_id = $1",
        )
        .bind(track_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            tracing::error!("{e}");
            anyhow::anyhow!("Unable to get track play statistics: {e}")
        })
    }

    /// Count one play of `kind` against the track's row, creating the row on
    /// the first report. One statement does it: the insert carries the
    /// kind's triple, and the conflict arm adds the same triple to the row's
    /// own counters.
    ///
    /// # Errors
    /// If the database query fails, return an error
    #[cfg(feature = "server")]
    pub async fn record_play(pool: &PgPool, track_id: i64, kind: PlayKind) -> anyhow::Result<()> {
        let (complete, partial, skip) = kind.counters();
        sqlx::query(
            "INSERT INTO track_play_stats \
                (track_id, complete_plays, partial_plays, skips) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (track_id) DO UPDATE SET \
                complete_plays = track_play_stats.complete_plays + $2, \
                partial_plays = track_play_stats.partial_plays + $3, \
                skips = track_play_stats.skips + $4, \
                updated_at = NOW ()",
        )
        .bind(track_id)
        .bind(complete)
        .bind(partial)
        .bind(skip)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("{e}");
            anyhow::anyhow!("Unable to record a track play: {e}")
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{PlayKind, TrackPlayStats};
    #[cfg(feature = "server")]
    use crate::models::test_helpers::create_test_track;
    #[cfg(feature = "server")]
    use sqlx::PgPool;

    #[sqlx::test]
    async fn test_record_play_counts(pool: PgPool) {
        let track = create_test_track(&pool, 1, None, None)
            .await
            .expect("test track");
        assert_eq!(
            TrackPlayStats::get_by_track_id(&pool, track.id)
                .await
                .expect("readable"),
            None,
            "no play has been counted for a fresh track"
        );

        TrackPlayStats::record_play(&pool, track.id, PlayKind::Complete)
            .await
            .expect("first complete play");
        TrackPlayStats::record_play(&pool, track.id, PlayKind::Complete)
            .await
            .expect("second complete play");
        TrackPlayStats::record_play(&pool, track.id, PlayKind::Partial)
            .await
            .expect("a partial play");
        TrackPlayStats::record_play(&pool, track.id, PlayKind::Skip)
            .await
            .expect("a skip");

        let stats = TrackPlayStats::get_by_track_id(&pool, track.id)
            .await
            .expect("readable")
            .expect("a counted track has a row");
        assert_eq!(stats.track_id, track.id);
        assert_eq!(stats.complete_plays, 2, "the upsert adds to its own column");
        assert_eq!(stats.partial_plays, 1);
        assert_eq!(stats.skips, 1, "and leaves the others alone");
    }

    #[sqlx::test]
    async fn test_stats_go_with_the_track(pool: PgPool) {
        // The foreign key is `ON DELETE CASCADE`: a hard-deleted track takes
        // its counters with it, so the table never accumulates orphans.
        let track = create_test_track(&pool, 1, None, None)
            .await
            .expect("test track");
        TrackPlayStats::record_play(&pool, track.id, PlayKind::Skip)
            .await
            .expect("a counted play");
        sqlx::query("DELETE FROM tracks WHERE id = $1")
            .bind(track.id)
            .execute(&pool)
            .await
            .expect("hard delete");
        assert_eq!(
            TrackPlayStats::get_by_track_id(&pool, track.id)
                .await
                .expect("readable"),
            None,
            "the counters went with the row"
        );
    }
}
