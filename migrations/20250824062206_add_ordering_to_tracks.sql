-- Add migration script here
ALTER TABLE tracks ADD COLUMN track_number INT;
ALTER TABLE tracks ADD COLUMN release_id BIGINT;
ALTER TABLE tracks ADD CONSTRAINT unique_release_track UNIQUE (release_id, track_number);
ALTER TABLE tracks ADD CONSTRAINT fk_releases FOREIGN KEY (release_id) REFERENCES releases(id);

DROP TABLE release_tracks;
