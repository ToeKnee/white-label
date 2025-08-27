-- Add migration script here
ALTER TABLE tracks ADD COLUMN lyrics TEXT NOT NULL DEFAULT '';
