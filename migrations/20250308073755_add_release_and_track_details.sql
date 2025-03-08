-- Add migration script here
BEGIN TRANSACTION;

ALTER TABLE releases
ADD COLUMN primary_image VARCHAR(255),
ADD COLUMN catalogue_number VARCHAR(255),
ADD COLUMN release_date TIMESTAMP
WITH
    TIME ZONE;

CREATE UNIQUE INDEX IF NOT EXISTS releases_catalog_number ON releases (catalogue_number, label_id);

ALTER TABLE tracks
ADD COLUMN primary_image VARCHAR(255);

COMMIT;
