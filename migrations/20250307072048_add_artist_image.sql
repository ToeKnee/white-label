-- Add migration script here
ALTER TABLE artists
ADD COLUMN primary_image VARCHAR(255);
