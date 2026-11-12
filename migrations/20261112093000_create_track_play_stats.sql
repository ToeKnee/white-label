-- Per-track play counters, fed by the global player's reports (see
-- `crate::ui::player` and `crate::services::track_play_stats`). A row
-- appears when the track's first counted play lands, and the three counters
-- say how the reported plays ended: `complete_plays` past the 90% mark
-- (including tracks that ended by themselves), `partial_plays` past 10% but
-- stopped before 90%, and `skips` stopped before 10%. A `label_owner`
-- session's plays are deliberately not counted, so these measure the
-- audience rather than the owner's previews.
CREATE TABLE IF NOT EXISTS track_play_stats (
    track_id BIGINT PRIMARY KEY,
    complete_plays INTEGER NOT NULL DEFAULT 0,
    partial_plays INTEGER NOT NULL DEFAULT 0,
    skips INTEGER NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ DEFAULT NOW (),
    FOREIGN KEY (track_id) REFERENCES tracks (id) ON DELETE CASCADE
);
