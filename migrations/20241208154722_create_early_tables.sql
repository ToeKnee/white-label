-- Add migration script here
CREATE TABLE IF NOT EXISTS labels (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(255) NOT NULL,
    description TEXT,
    isrc_base VARCHAR(12),
    created_at TIMESTAMPTZ DEFAULT NOW (),
    updated_at TIMESTAMPTZ DEFAULT NOW ()
);

CREATE UNIQUE INDEX IF NOT EXISTS labels_name_index ON labels (name);

CREATE UNIQUE INDEX IF NOT EXISTS labels_slug_index ON labels (slug);

CREATE TABLE IF NOT EXISTS artists (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(255) NOT NULL,
    description TEXT,
    label_id BIGINT,
    created_at TIMESTAMPTZ DEFAULT NOW (),
    updated_at TIMESTAMPTZ DEFAULT NOW (),
    FOREIGN KEY (label_id) REFERENCES labels (id) ON DELETE SET NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS artists_name_index ON artists (name);

CREATE UNIQUE INDEX IF NOT EXISTS artists_slug_index ON artists (slug);

CREATE TABLE IF NOT EXISTS releases (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(255) NOT NULL,
    description TEXT,
    label_id BIGINT,
    created_at TIMESTAMPTZ DEFAULT NOW (),
    updated_at TIMESTAMPTZ DEFAULT NOW (),
    FOREIGN KEY (label_id) REFERENCES labels (id) ON DELETE SET NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS releases_name_index ON releases (name);

CREATE UNIQUE INDEX IF NOT EXISTS releases_slug_index ON releases (slug);

CREATE TABLE IF NOT EXISTS release_artists (
    id BIGSERIAL PRIMARY KEY,
    artist_id BIGINT NOT NULL,
    release_id BIGINT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW (),
    updated_at TIMESTAMPTZ DEFAULT NOW (),
    FOREIGN KEY (artist_id) REFERENCES artists (id) ON DELETE CASCADE,
    FOREIGN KEY (release_id) REFERENCES releases (id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS release_artists_artist_id_release_id_index ON release_artists (artist_id, release_id);

CREATE TABLE IF NOT EXISTS tracks (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(255) NOT NULL,
    description TEXT,
    isrc_code VARCHAR(12) NOT NULL,
    bpm INT,
    created_at TIMESTAMPTZ DEFAULT NOW (),
    updated_at TIMESTAMPTZ DEFAULT NOW ()
);

CREATE UNIQUE INDEX IF NOT EXISTS tracks_isrc_code_index ON tracks (isrc_code);

CREATE UNIQUE INDEX IF NOT EXISTS tracks_slug_index ON tracks (slug);

CREATE TABLE IF NOT EXISTS track_artists (
    id BIGSERIAL PRIMARY KEY,
    artist_id BIGINT NOT NULL,
    track_id BIGINT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW (),
    updated_at TIMESTAMPTZ DEFAULT NOW (),
    FOREIGN KEY (artist_id) REFERENCES artists (id) ON DELETE CASCADE,
    FOREIGN KEY (track_id) REFERENCES tracks (id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS track_artists_artist_id_track_id_index ON track_artists (artist_id, track_id);

CREATE TABLE IF NOT EXISTS release_tracks (
    id BIGSERIAL PRIMARY KEY,
    track_id BIGINT NOT NULL,
    release_id BIGINT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW (),
    updated_at TIMESTAMPTZ DEFAULT NOW (),
    FOREIGN KEY (track_id) REFERENCES tracks (id) ON DELETE CASCADE,
    FOREIGN KEY (release_id) REFERENCES releases (id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS release_tracks_track_id_release_id_index ON release_tracks (track_id, release_id);
