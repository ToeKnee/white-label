-- Add migration script here
ALTER TABLE users
ADD COLUMN avatar VARCHAR(255);
