-- Add migration script here
ALTER TABLE users
ADD COLUMN first_name VARCHAR(255);

ALTER TABLE users
ADD COLUMN last_name VARCHAR(255);

ALTER TABLE users
ADD COLUMN email VARCHAR(255);

UPDATE users
SET
    email = username
WHERE
    email IS NULL;

ALTER TABLE users
ALTER COLUMN email
SET
    NOT NULL;

ALTER TABLE users ADD UNIQUE (email);

ALTER TABLE users
ADD COLUMN description TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS users_email_index ON users (email);
