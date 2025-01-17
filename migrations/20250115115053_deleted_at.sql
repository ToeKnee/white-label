-- Add migration script here
ALTER TABLE artists
ADD COLUMN deleted_at TIMESTAMP
WITH
    TIME ZONE;

-- Add deleted_at column to the releases table
ALTER TABLE releases
ADD COLUMN deleted_at TIMESTAMP
WITH
    TIME ZONE;

-- Add deleted_at column to the tracks table
ALTER TABLE tracks
ADD COLUMN deleted_at TIMESTAMP
WITH
    TIME ZONE;
