-- Add migration script here
BEGIN TRANSACTION;

ALTER TABLE releases
ADD COLUMN primary_artist_id BIGINT;

ALTER TABLE releases ADD CONSTRAINT primary_artist_id FOREIGN KEY (primary_artist_id) REFERENCES artists (id) ON DELETE SET NULL;

ALTER TABLE tracks
ADD COLUMN primary_artist_id BIGINT;

ALTER TABLE tracks ADD CONSTRAINT primary_artist_id FOREIGN KEY (primary_artist_id) REFERENCES artists (id) ON DELETE SET NULL;

COMMIT;
