-- Add migration script here
-- How many times one guest browser may play each track before its rows show
-- "No plays left" (signed-in listeners are exempt). The Rust-side default
-- (`RecordLabel::DEFAULT_GUEST_PLAYS`) must match the 5 here.
ALTER TABLE labels ADD COLUMN guest_plays INTEGER NOT NULL DEFAULT 5;
