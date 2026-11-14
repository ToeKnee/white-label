//! Per-track play counters, stored one row per track **per day**.
//!
//! The global player reports each play it stops (see [`crate::ui::player`]'s
//! reporting), and [`crate::services::track_play_stats`] decides which
//! reports count before one of the three counters of that day's row is
//! incremented. The per-day shape is what lets the stats pages
//! ([`crate::services::stats`]) chart the counters over time; the day a
//! report lands on is the database's own `CURRENT_DATE`.

use chrono::NaiveDate;
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

/// The counters stored for one track on one day. `updated_at` is
/// deliberately not a field: the upsert maintains the column, and no read
/// shows it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(FromRow))]
pub struct TrackPlayStats {
    /// The track these counters belong to.
    pub track_id: i64,
    /// The day these counters accumulated on.
    pub day: NaiveDate,
    /// Plays that passed the 90% mark.
    pub complete_plays: i32,
    /// Plays that passed 10% but stopped before 90%.
    pub partial_plays: i32,
    /// Plays stopped before the 10% mark.
    pub skips: i32,
}

impl TrackPlayStats {
    /// The counters stored for `track_id` on `day`, or `None` when no
    /// counted play landed for it that day.
    ///
    /// # Errors
    /// If the database query fails, return an error
    #[cfg(feature = "server")]
    pub async fn get_for_day(
        pool: &PgPool,
        track_id: i64,
        day: NaiveDate,
    ) -> anyhow::Result<Option<Self>> {
        sqlx::query_as::<_, Self>(
            "SELECT track_id, day, complete_plays, partial_plays, skips \
             FROM track_play_stats WHERE track_id = $1 AND day = $2",
        )
        .bind(track_id)
        .bind(day)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            tracing::error!("{e}");
            anyhow::anyhow!("Unable to get track play statistics: {e}")
        })
    }

    /// Count one play of `kind` against the track's row for the current
    /// day, creating the row on the first report that day. One statement
    /// does it: the insert carries the kind's triple, and the conflict arm
    /// adds the same triple to the day's own counters.
    ///
    /// # Errors
    /// If the database query fails, return an error
    #[cfg(feature = "server")]
    pub async fn record_play(pool: &PgPool, track_id: i64, kind: PlayKind) -> anyhow::Result<()> {
        let (complete, partial, skip) = kind.counters();
        sqlx::query(
            "INSERT INTO track_play_stats \
                (track_id, day, complete_plays, partial_plays, skips) \
             VALUES ($1, CURRENT_DATE, $2, $3, $4) \
             ON CONFLICT (track_id, day) DO UPDATE SET \
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

/// One day's counters for a scope (a label, artist or release): the points
/// of a stats page's graph.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(FromRow))]
pub struct DayTotals {
    /// The day these counters accumulated on.
    pub day: NaiveDate,
    /// Plays that passed the 90% mark.
    pub complete_plays: i32,
    /// Plays that passed 10% but stopped before 90%.
    pub partial_plays: i32,
    /// Plays stopped before the 10% mark.
    pub skips: i32,
}

/// One track's counters within a stats page's scope and range.
///
/// Carries the slugs its table row links to the track editor with, plus
/// the artist and release names the row shows. Tracks with no counted
/// plays are included (all-zero counters) so a scope's table shows the
/// whole catalogue, not just what has been heard.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(FromRow))]
pub struct TrackTotals {
    /// The track's primary artist's slug.
    pub artist_slug: String,
    /// The track's primary artist's name.
    pub artist_name: String,
    /// The release the track sits on.
    pub release_slug: String,
    /// The release's name.
    pub release_name: String,
    /// The track's slug.
    pub track_slug: String,
    /// The track's name (what the row shows).
    pub track_name: String,
    /// Plays that passed the 90% mark.
    pub complete_plays: i32,
    /// Plays that passed 10% but stopped before 90%.
    pub partial_plays: i32,
    /// Plays stopped before the 10% mark.
    pub skips: i32,
}

/// How a stats table's rows are ordered, addressed by the code its column
/// header puts in the URL.
///
/// The code, the header label and the `ORDER BY` clause live here, so a page
/// can offer the six columns without a second list that could drift, and
/// nothing a visitor sends ever reaches the SQL: an unknown code is refused
/// before the query is built (see [`crate::services::stats`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaySort {
    /// Track name, A to Z.
    Track,
    /// Primary artist name, A to Z.
    Artist,
    /// Release name, A to Z.
    Release,
    /// Most skipped first.
    Skips,
    /// Most partially heard first.
    Partial,
    /// Most completed first — the default, and what a page shows when its URL
    /// carries no sort.
    Complete,
}

impl PlaySort {
    /// The six sorts in column order, each with the code its header link
    /// carries.
    pub const COLUMNS: [(&'static str, &'static str); 6] = [
        ("track", "Track"),
        ("artist", "Artist"),
        ("release", "Release"),
        ("skips", "Skips"),
        ("partial", "Partial"),
        ("complete", "Complete"),
    ];

    /// The sort a code names: `None` for an unknown code, `Some(Complete)`
    /// for an empty one (a page whose URL carries no sort means the default).
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "" | "complete" => Some(Self::Complete),
            "track" => Some(Self::Track),
            "artist" => Some(Self::Artist),
            "release" => Some(Self::Release),
            "skips" => Some(Self::Skips),
            "partial" => Some(Self::Partial),
            _ => None,
        }
    }

    /// The `ORDER BY` this sort means. Ties fall back to track name, so two
    /// loads of the same page agree row for row.
    #[must_use]
    pub const fn order_clause(&self) -> &'static str {
        match self {
            Self::Track => "t.name ASC",
            Self::Artist => "pa.name ASC, t.name ASC",
            Self::Release => "r.name ASC, t.name ASC",
            Self::Skips => "COALESCE (SUM (s.skips), 0) DESC, t.name ASC",
            Self::Partial => "COALESCE (SUM (s.partial_plays), 0) DESC, t.name ASC",
            Self::Complete => "COALESCE (SUM (s.complete_plays), 0) DESC, t.name ASC",
        }
    }
}

/// Which catalogue a stats page covers, addressed by row id.
///
/// Both queries drop soft-deleted rows (their pages are gone). A track
/// belongs to an artist scope when that artist is its primary artist (as on
/// the public artist pages) or one of its `track_artists` links (a featured
/// appearance), so a track is never lost to a missing link row.
#[cfg(feature = "server")]
#[derive(Clone, Copy, Debug)]
pub enum StatsScope {
    /// Every release under the label with this id.
    Label(i64),
    /// Every track by the artist with this id, primary or featured.
    Artist(i64),
    /// Every track on the release with this id.
    Release(i64),
}

#[cfg(feature = "server")]
impl StatsScope {
    /// The `WHERE` filter each scope needs, shared by both queries below
    /// (`$1` is the scope's row id in every case). No scope joins a
    /// many-to-many table: the artist test is a subquery, so a track with
    /// several artist links still costs one row.
    const fn filter(&self) -> &'static str {
        match self {
            Self::Label(_) => "r.label_id = $1 AND t.deleted_at IS NULL AND r.deleted_at IS NULL",
            Self::Artist(_) => {
                "(t.primary_artist_id = $1 \
                OR EXISTS (SELECT 1 FROM track_artists ta \
                    WHERE ta.track_id = t.id AND ta.artist_id = $1)) \
                AND t.deleted_at IS NULL AND r.deleted_at IS NULL"
            }
            Self::Release(_) => {
                "t.release_id = $1 AND t.deleted_at IS NULL AND r.deleted_at IS NULL"
            }
        }
    }

    /// The scope's counters grouped by day, oldest first. `since` cuts the
    /// window at its first day; `None` covers every day on file.
    ///
    /// # Errors
    /// If the database query fails, return an error
    pub async fn day_series(
        &self,
        pool: &PgPool,
        since: Option<NaiveDate>,
    ) -> anyhow::Result<Vec<DayTotals>> {
        let filter = self.filter();
        let query = format!(
            "SELECT s.day, \
                SUM (s.complete_plays)::INTEGER AS complete_plays, \
                SUM (s.partial_plays)::INTEGER AS partial_plays, \
                SUM (s.skips)::INTEGER AS skips \
             FROM track_play_stats s \
             JOIN tracks t ON t.id = s.track_id \
             JOIN releases r ON r.id = t.release_id \
             WHERE {filter} AND s.day >= COALESCE ($2::DATE, s.day) \
             GROUP BY s.day \
             ORDER BY s.day ASC"
        );
        // Audited for sqlx's injection guard: the interpolated `filter` is a
        // const `&'static str` fragment (see [`StatsScope::filter`]); the
        // scope id and `since` are bind parameters.
        sqlx::query_as::<_, DayTotals>(sqlx::AssertSqlSafe(query.as_str()))
            .bind(match self {
                Self::Label(id) | Self::Artist(id) | Self::Release(id) => *id,
            })
            .bind(since)
            .fetch_all(pool)
            .await
            .map_err(|e| {
                tracing::error!("{e}");
                anyhow::anyhow!("Unable to read the play statistics graph: {e}")
            })
    }

    /// The scope's tracks with their counters summed over the range, in the
    /// order `sort` names. `since` cuts the window at its first day; `None`
    /// covers every day on file. The `track_play_stats` join is `LEFT`, so
    /// tracks nobody has heard still appear (with zeroes).
    ///
    /// # Errors
    /// If the database query fails, return an error
    pub async fn track_totals(
        &self,
        pool: &PgPool,
        since: Option<NaiveDate>,
        sort: PlaySort,
    ) -> anyhow::Result<Vec<TrackTotals>> {
        let filter = self.filter();
        let order = sort.order_clause();
        let query = format!(
            "SELECT pa.slug AS artist_slug, pa.name AS artist_name, \
                r.slug AS release_slug, r.name AS release_name, \
                t.slug AS track_slug, t.name AS track_name, \
                COALESCE (SUM (s.complete_plays), 0)::INTEGER AS complete_plays, \
                COALESCE (SUM (s.partial_plays), 0)::INTEGER AS partial_plays, \
                COALESCE (SUM (s.skips), 0)::INTEGER AS skips \
             FROM tracks t \
             JOIN releases r ON r.id = t.release_id \
             JOIN artists pa ON pa.id = t.primary_artist_id \
             LEFT JOIN track_play_stats s \
                ON s.track_id = t.id AND s.day >= COALESCE ($2::DATE, s.day) \
             WHERE {filter} \
             GROUP BY t.id, pa.slug, pa.name, r.slug, r.name, t.slug, t.name \
             ORDER BY {order}"
        );
        // Audited for sqlx's injection guard: the interpolated `filter` and
        // `order` are const `&'static str` fragments (see
        // [`StatsScope::filter`] and [`PlaySort::order_clause`]); the scope
        // id and `since` are bind parameters.
        sqlx::query_as::<_, TrackTotals>(sqlx::AssertSqlSafe(query.as_str()))
            .bind(match self {
                Self::Label(id) | Self::Artist(id) | Self::Release(id) => *id,
            })
            .bind(since)
            .fetch_all(pool)
            .await
            .map_err(|e| {
                tracing::error!("{e}");
                anyhow::anyhow!("Unable to read the play statistics table: {e}")
            })
    }
}

#[cfg(test)]
mod tests {
    use super::PlaySort;
    #[cfg(feature = "server")]
    use super::{DayTotals, PlayKind, StatsScope, TrackPlayStats, TrackTotals};
    #[cfg(feature = "server")]
    use crate::models::test_helpers::{
        create_test_artist, create_test_record_label, create_test_release, create_test_track,
    };
    #[cfg(feature = "server")]
    use sqlx::PgPool;

    /// Today, in the same clock the `CURRENT_DATE` default uses.
    #[cfg(feature = "server")]
    fn today() -> chrono::NaiveDate {
        chrono::Utc::now().date_naive()
    }

    #[cfg(feature = "server")]
    #[sqlx::test]
    async fn test_record_play_counts_one_row_per_day(pool: PgPool) {
        let track = create_test_track(&pool, 1, None, None)
            .await
            .expect("test track");
        assert_eq!(
            TrackPlayStats::get_for_day(&pool, track.id, today())
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

        let stats = TrackPlayStats::get_for_day(&pool, track.id, today())
            .await
            .expect("readable")
            .expect("a counted day has a row");
        assert_eq!(stats.track_id, track.id);
        assert_eq!(stats.day, today(), "the row is today's");
        assert_eq!(stats.complete_plays, 2, "the upsert adds to its own column");
        assert_eq!(stats.partial_plays, 1);
        assert_eq!(stats.skips, 1, "and leaves the others alone");

        // A fresh day is a fresh row: yesterday's counters stay at zero.
        assert_eq!(
            TrackPlayStats::get_for_day(&pool, track.id, today() - chrono::Duration::days(1))
                .await
                .expect("readable"),
            None
        );
    }

    #[cfg(feature = "server")]
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
        let counters_left: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM track_play_stats WHERE track_id = $1")
                .bind(track.id)
                .fetch_one(&pool)
                .await
                .expect("readable");
        assert_eq!(counters_left, 0, "the counters went with the rows");
    }

    #[cfg(feature = "server")]
    #[sqlx::test]
    async fn test_day_series_spans_the_range(pool: PgPool) {
        // A row for a day record_play cannot produce (the upsert always
        // lands on CURRENT_DATE), inserted directly: the series and its
        // `since` window are what the stats pages read.
        let track = create_test_track(&pool, 1, None, None)
            .await
            .expect("test track");
        let past = today() - chrono::Duration::days(40);
        sqlx::query(
            "INSERT INTO track_play_stats (track_id, day, complete_plays) VALUES ($1, $2, 1)",
        )
        .bind(track.id)
        .bind(past)
        .execute(&pool)
        .await
        .expect("a past day's row");
        TrackPlayStats::record_play(&pool, track.id, PlayKind::Skip)
            .await
            .expect("today's skip");

        let scope = StatsScope::Release(track.release_id);
        let all = scope.day_series(&pool, None).await.expect("series");
        assert_eq!(
            all,
            vec![
                DayTotals {
                    day: past,
                    complete_plays: 1,
                    partial_plays: 0,
                    skips: 0,
                },
                DayTotals {
                    day: today(),
                    complete_plays: 0,
                    partial_plays: 0,
                    skips: 1,
                },
            ],
            "both days, oldest first"
        );

        let window = scope
            .day_series(&pool, Some(today() - chrono::Duration::days(6)))
            .await
            .expect("series");
        assert_eq!(window.len(), 1, "the past day falls outside a 7-day cut");
        assert_eq!(window[0].day, today());

        // The table over the whole range adds the two days together.
        let rows = scope
            .track_totals(&pool, None, PlaySort::Complete)
            .await
            .expect("table");
        assert_eq!(
            rows,
            vec![TrackTotals {
                artist_slug: "test-artist-1".to_string(),
                artist_name: "Test Artist 1".to_string(),
                release_slug: "test-release-1".to_string(),
                release_name: "Test Release 1".to_string(),
                track_slug: "test-track-1".to_string(),
                track_name: "Test Track 1".to_string(),
                complete_plays: 1,
                partial_plays: 0,
                skips: 1,
            }],
            "every day on file adds up"
        );
    }

    #[cfg(feature = "server")]
    #[sqlx::test]
    async fn test_scopes_see_live_rows_only(pool: PgPool) {
        // One label, one artist, two releases. The counted track shares its
        // release with a soft-deleted track (hidden from every scope); an
        // uncounted track on the artist's other release still shows, with
        // zeroes, at the artist and label levels.
        let label = create_test_record_label(&pool, 1)
            .await
            .expect("label fixture");
        let label_id = label.id;
        let artist = create_test_artist(&pool, 1, Some(label))
            .await
            .expect("artist fixture");
        let artist_id = artist.id;
        let release1 = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .expect("release fixture");
        let release2 = create_test_release(&pool, 2, Some(artist.clone()))
            .await
            .expect("second release");
        let counted = create_test_track(&pool, 1, Some(release1.clone()), Some(artist.clone()))
            .await
            .expect("counted track");
        let deleted = create_test_track(&pool, 2, Some(release1), Some(artist.clone()))
            .await
            .expect("doomed track");
        create_test_track(&pool, 3, Some(release2), Some(artist))
            .await
            .expect("uncounted track");

        TrackPlayStats::record_play(&pool, counted.id, PlayKind::Complete)
            .await
            .expect("a counted play");
        deleted.delete(&pool).await.expect("soft delete");

        let release_rows = StatsScope::Release(counted.release_id)
            .track_totals(&pool, None, PlaySort::Track)
            .await
            .expect("release scope");
        assert_eq!(release_rows.len(), 1, "the deleted track is hidden");
        assert_eq!(
            release_rows[0],
            TrackTotals {
                artist_slug: "test-artist-1".to_string(),
                artist_name: "Test Artist 1".to_string(),
                release_slug: "test-release-1".to_string(),
                release_name: "Test Release 1".to_string(),
                track_slug: "test-track-1".to_string(),
                track_name: "Test Track 1".to_string(),
                complete_plays: 1,
                partial_plays: 0,
                skips: 0,
            },
            "and the editor link slugs come along"
        );

        for scope in [StatsScope::Artist(artist_id), StatsScope::Label(label_id)] {
            let rows = scope
                .track_totals(&pool, None, PlaySort::Track)
                .await
                .expect("scope");
            let names: Vec<&str> = rows.iter().map(|row| row.track_name.as_str()).collect();
            assert_eq!(
                names,
                ["Test Track 1", "Test Track 3"],
                "{scope:?} shows the live catalogue by name, counted or not"
            );
        }

        // A range cut leaves a track that was heard only before it on the
        // table (with zeroes) but drops it from the graph.
        let window = StatsScope::Release(counted.release_id)
            .track_totals(
                &pool,
                Some(today() + chrono::Duration::days(1)),
                PlaySort::Track,
            )
            .await
            .expect("future-cut window");
        assert_eq!(window.len(), 1, "the live track still shows");
        assert_eq!(
            (
                window[0].complete_plays,
                window[0].partial_plays,
                window[0].skips
            ),
            (0, 0, 0),
            "its counters fall outside the cut"
        );
    }

    #[cfg(feature = "server")]
    #[sqlx::test]
    async fn test_artist_scope_covers_primary_and_featured(pool: PgPool) {
        /// The track names, in the order the table's rows arrive.
        fn track_names(rows: &[TrackTotals]) -> Vec<&str> {
            rows.iter().map(|row| row.track_name.as_str()).collect()
        }

        // The track editors keep `primary_artist_id` and the `track_artists`
        // links together, but a row can carry only one of them (a fixture
        // seeded straight into SQL, or an import). The artist scope accepts
        // either way in, once per track, and never adopts a stranger's
        // catalogue.
        let label = create_test_record_label(&pool, 1)
            .await
            .expect("label fixture");
        let lead = create_test_artist(&pool, 1, Some(label.clone()))
            .await
            .expect("lead artist fixture");
        let guest = create_test_artist(&pool, 2, Some(label))
            .await
            .expect("guest artist fixture");
        let release = create_test_release(&pool, 1, Some(lead.clone()))
            .await
            .expect("release fixture");

        // Primary artist, with and without the usual link row.
        let linked = create_test_track(&pool, 1, Some(release.clone()), Some(lead.clone()))
            .await
            .expect("linked track");
        let unlinked = create_test_track(&pool, 2, Some(release.clone()), Some(lead.clone()))
            .await
            .expect("unlinked track");
        sqlx::query("DELETE FROM track_artists WHERE track_id = $1")
            .bind(unlinked.id)
            .execute(&pool)
            .await
            .expect("dropping the link row");
        // A featured spot: the link names the guest artist, while the track
        // stays primary to the release's artist.
        let featured = create_test_track(&pool, 3, Some(release.clone()), Some(guest.clone()))
            .await
            .expect("featured track");
        // The guest artist's own release, which the lead sees no part of.
        let guest_release = create_test_release(&pool, 2, Some(guest.clone()))
            .await
            .expect("guest release");
        create_test_track(&pool, 4, Some(guest_release), Some(guest.clone()))
            .await
            .expect("guest track");

        let lead_rows = StatsScope::Artist(lead.id)
            .track_totals(&pool, None, PlaySort::Track)
            .await
            .expect("lead scope");
        assert_eq!(
            track_names(&lead_rows),
            ["Test Track 1", "Test Track 2", "Test Track 3"],
            "primary with a link, primary without one, and a featured spot \
             on the lead's own release — each once"
        );

        let guest_rows = StatsScope::Artist(guest.id)
            .track_totals(&pool, None, PlaySort::Track)
            .await
            .expect("guest scope");
        assert_eq!(
            track_names(&guest_rows),
            ["Test Track 3", "Test Track 4"],
            "the guest sees its featured spot and its own release"
        );

        // The graph goes through the same filter, so a track no link row
        // names still contributes its counters.
        TrackPlayStats::record_play(&pool, linked.id, PlayKind::Partial)
            .await
            .expect("a play on the linked track");
        TrackPlayStats::record_play(&pool, unlinked.id, PlayKind::Skip)
            .await
            .expect("a play on the unlinked track");
        TrackPlayStats::record_play(&pool, featured.id, PlayKind::Complete)
            .await
            .expect("a play on the featured track");

        let lead_series = StatsScope::Artist(lead.id)
            .day_series(&pool, None)
            .await
            .expect("lead series");
        assert_eq!(
            lead_series,
            vec![DayTotals {
                day: today(),
                complete_plays: 1,
                partial_plays: 1,
                skips: 1,
            }],
            "one of each kind, from a linked track, an unlinked one and a \
             featured spot"
        );

        let guest_series = StatsScope::Artist(guest.id)
            .day_series(&pool, None)
            .await
            .expect("guest series");
        assert_eq!(
            guest_series,
            vec![DayTotals {
                day: today(),
                complete_plays: 1,
                partial_plays: 0,
                skips: 0,
            }],
            "the guest counts only its featured spot today"
        );
    }

    #[test]
    fn the_sort_codes_are_the_table_columns() {
        // The stats pages build their column headers from `COLUMNS`, so the
        // codes and labels have to stay in step with `from_code`.
        assert_eq!(
            PlaySort::COLUMNS,
            [
                ("track", "Track"),
                ("artist", "Artist"),
                ("release", "Release"),
                ("skips", "Skips"),
                ("partial", "Partial"),
                ("complete", "Complete"),
            ]
        );
        for (code, _label) in PlaySort::COLUMNS {
            assert!(PlaySort::from_code(code).is_some(), "{code} names a sort");
        }
        assert_eq!(
            PlaySort::from_code(""),
            Some(PlaySort::Complete),
            "a page with no sort in its URL gets the default"
        );
        assert_eq!(
            PlaySort::from_code("sideways"),
            None,
            "unknown codes are refused"
        );
    }

    #[cfg(feature = "server")]
    #[sqlx::test]
    async fn test_track_totals_follow_the_sort(pool: PgPool) {
        // Four tracks on one release, heard four different ways: each column
        // leads with a different track, and equal counters fall back to name
        // order so two loads of the page agree row for row. Track 2 is moved
        // to a second primary artist, so the artist column has its own
        // order to prove.
        let release = create_test_release(&pool, 1, None)
            .await
            .expect("release fixture");
        let second_artist = create_test_artist(&pool, 2, None)
            .await
            .expect("second artist fixture");
        let mut heard = Vec::new();
        for id in 1..=4 {
            heard.push(
                create_test_track(&pool, id, Some(release.clone()), None)
                    .await
                    .expect("track fixture"),
            );
        }
        sqlx::query("UPDATE tracks SET primary_artist_id = $1 WHERE id = $2")
            .bind(second_artist.id)
            .bind(heard[1].id)
            .execute(&pool)
            .await
            .expect("move track 2 to the second artist");
        for _ in 0..3 {
            TrackPlayStats::record_play(&pool, heard[0].id, PlayKind::Skip)
                .await
                .expect("a skip");
        }
        for _ in 0..2 {
            TrackPlayStats::record_play(&pool, heard[1].id, PlayKind::Partial)
                .await
                .expect("a partial play");
        }
        TrackPlayStats::record_play(&pool, heard[2].id, PlayKind::Complete)
            .await
            .expect("a complete play");
        for _ in 0..5 {
            TrackPlayStats::record_play(&pool, heard[3].id, PlayKind::Skip)
                .await
                .expect("a skip");
        }

        let scope = StatsScope::Release(release.id);
        let cases = [
            (
                PlaySort::Complete,
                [
                    "Test Track 3",
                    "Test Track 1",
                    "Test Track 2",
                    "Test Track 4",
                ],
            ),
            (
                PlaySort::Partial,
                [
                    "Test Track 2",
                    "Test Track 1",
                    "Test Track 3",
                    "Test Track 4",
                ],
            ),
            (
                PlaySort::Skips,
                [
                    "Test Track 4",
                    "Test Track 1",
                    "Test Track 2",
                    "Test Track 3",
                ],
            ),
            (
                PlaySort::Track,
                [
                    "Test Track 1",
                    "Test Track 2",
                    "Test Track 3",
                    "Test Track 4",
                ],
            ),
            (
                // The release artist leads with the three tracks still under
                // it; the second artist's track falls in by name.
                PlaySort::Artist,
                [
                    "Test Track 1",
                    "Test Track 3",
                    "Test Track 4",
                    "Test Track 2",
                ],
            ),
            (
                // One release: every row ties, so names decide again.
                PlaySort::Release,
                [
                    "Test Track 1",
                    "Test Track 2",
                    "Test Track 3",
                    "Test Track 4",
                ],
            ),
        ];
        for (sort, expected) in cases {
            let rows = scope
                .track_totals(&pool, None, sort)
                .await
                .expect("the table");
            let names: Vec<&str> = rows.iter().map(|row| row.track_name.as_str()).collect();
            assert_eq!(names, expected, "{sort:?} puts its own column first");
        }
    }
}
