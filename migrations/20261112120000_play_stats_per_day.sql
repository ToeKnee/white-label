-- Play counters move from one row per track to one row per track per day,
-- so the stats pages can chart how the counters moved over time.
--
-- The single-row-per-track table is dropped rather than backfilled: its
-- totals cannot be spread over days honestly, and the counters only ever
-- carried development traffic (the feature shipped in the same batch).

DROP TABLE IF EXISTS track_play_stats;

CREATE TABLE track_play_stats (
    track_id BIGINT NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    -- The local day the play landed on (`CURRENT_DATE` at insert time).
    day DATE NOT NULL DEFAULT CURRENT_DATE,
    complete_plays INTEGER NOT NULL DEFAULT 0,
    partial_plays INTEGER NOT NULL DEFAULT 0,
    skips INTEGER NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW (),
    PRIMARY KEY (track_id, day)
);
