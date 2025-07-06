-- Add migration script here

CREATE TYPE SocialMedia AS ENUM (
    'BlueSky',
    'Facebook',
    'Instagram',
    'LinkedIn',
    'Mastodon',
    'Pinterest',
    'Snapchat',
    'Threads',
    'TikTok',
    'Twitter',
    'YouTube'
);

CREATE TABLE IF NOT EXISTS social_media (
    id BIGSERIAL PRIMARY KEY,
    artist_id BIGINT NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
    platform SocialMedia NOT NULL,
    url TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW (),
    updated_at TIMESTAMPTZ DEFAULT NOW ()
);


CREATE TYPE Platform AS ENUM (
    'AmazonMusic',
    'AppleMusic',
    'Bandcamp',
    'Beatport',
    'Deezer',
    'SoundCloud',
    'Spotify',
    'Tidal',
    'YouTubeMusic'
);

CREATE TABLE IF NOT EXISTS music_services (
    id BIGSERIAL PRIMARY KEY,
    artist_id BIGINT NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
    platform Platform NOT NULL,
    url TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW (),
    updated_at TIMESTAMPTZ DEFAULT NOW ()
);

ALTER TABLE artists
ADD COLUMN website TEXT DEFAULT '';
