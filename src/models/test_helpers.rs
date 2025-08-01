//! This module provides helper functions to create test data for artists, pages, record labels, releases, tracks,
//! and users in a `PostgreSQL` database using `SQLx`.
use sqlx::PgPool;

use crate::models::artist::Artist;
#[cfg(feature = "ssr")]
use crate::models::auth::{User, ssr::SqlPermissionTokens, ssr::SqlUser};
use crate::models::page::Page;
use crate::models::record_label::RecordLabel;
use crate::models::release::Release;
use crate::models::track::Track;

/// Create test artist
///
/// # Arguments
/// * `pool` - The database connection pool
/// * `id` - The ID of the artist
/// * `record_label` - The record label the artist is signed to (optional)
///
/// # Returns
/// The created artist
///
/// # Errors
/// If the artist cannot be created, return an error
///
/// # Panics
/// If the artist cannot be created, panic
/// If the record label is not found or cannot be created, panic
#[cfg(feature = "ssr")]
pub async fn create_test_artist(
    pool: &PgPool,
    id: usize,
    record_label: Option<RecordLabel>,
) -> Result<Artist, sqlx::Error> {
    let record_label = match record_label {
        Some(label) => label,
        None => create_test_record_label(pool, id).await.unwrap(),
    };
    let artist =
        sqlx::query_as::<_, Artist>("INSERT INTO artists (name, slug, description, website, label_id, published_at) VALUES ($1, $2, $3, $4, $5, $6) RETURNING *")
            .bind(format!("Test Artist {id}"))
            .bind(format!("test-artist-{id}"))
            .bind(format!("A artist for testing purposes with the id of {id}"))
            .bind(format!("https://test-artist-{id}.example.com"))
            .bind(record_label.id)
            .bind(Some(chrono::Utc::now()))
            .fetch_one(pool)
            .await?;

    Ok(artist)
}

/// Create test page
///
/// # Arguments
/// * `pool` - The database connection pool
/// * `id` - The ID of the page
/// * `record_label` - The record label the page is signed to (optional)
///
/// # Returns
/// The created page
///
/// # Errors
/// If the page cannot be created, return an error
///
/// # Panics
/// If the page cannot be created, panic
/// If the record label is not found or cannot be created, panic
#[cfg(feature = "ssr")]
pub async fn create_test_page(
    pool: &PgPool,
    id: usize,
    record_label: Option<RecordLabel>,
) -> Result<Page, sqlx::Error> {
    let record_label = match record_label {
        Some(label) => label,
        None => create_test_record_label(pool, id).await.unwrap(),
    };
    let page = sqlx::query_as::<_, Page>(
        "INSERT INTO pages (name, slug, description, body, label_id, published_at) VALUES ($1, $2, $3, $4, $5, $6) RETURNING *",
    )
    .bind(format!("Test Page {id}"))
    .bind(format!("test-page-{id}"))
    .bind(format!("A page for testing purposes with the id of {id}"))
    .bind(format!("# A page for testing purposes with the id of {id}"))
    .bind(record_label.id)
    .bind(Some(chrono::Utc::now()))
    .fetch_one(pool)
    .await?;

    Ok(page)
}

/// Create test record label
///
/// # Arguments
/// * `pool` - The database connection pool
/// * `id` - The ID of the record label
///
/// # Returns
/// The created record label
///
/// # Errors
/// If the record label cannot be created, return an error
///
/// # Panics
/// If the record label cannot be created, panic
#[cfg(feature = "ssr")]
pub async fn create_test_record_label(
    pool: &PgPool,
    id: usize,
) -> Result<RecordLabel, sqlx::Error> {
    let label = sqlx::query_as::<_, RecordLabel>("INSERT INTO labels (name, slug, description, isrc_base) VALUES ($1, $2, $3, $4) RETURNING *")
        .bind(format!("Test Record Label {id}"))
        .bind(format!("test-record-label-{id}"))
        .bind(format!("A record label for testing purposes with the id of {id}"))
        .bind(format!("UK AAA {id}"))
        .fetch_one(pool)
        .await?;

    Ok(label)
}

/// Create test release
///
/// # Arguments
/// * `pool` - The database connection pool
/// * `id` - The ID of the release
/// * `artist` - The artist the release is by (optional)
///
/// # Returns
/// The created release
///
/// # Errors
/// If the release cannot be created, return an error
///
/// # Panics
/// If the release cannot be created, panic
/// If the record label is not found or cannot be created, panic
#[cfg(feature = "ssr")]
pub async fn create_test_release(
    pool: &PgPool,
    id: usize,
    artist: Option<Artist>,
) -> Result<Release, sqlx::Error> {
    let artist = match artist {
        Some(artist) => artist,
        None => create_test_artist(pool, id, None).await.unwrap(),
    };

    let release =  sqlx::query_as::<_, Release>(
    "INSERT INTO releases (name, slug, description, primary_artist_id, catalogue_number, release_date, label_id, published_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING *",
    )
    .bind(format!("Test Release {id}"))
    .bind(format!("test-release-{id}"))
    .bind(format!("A release for testing purposes with the id of {id}"))
    .bind(artist.id)
    .bind(format!("TEST-{id}"))
    .bind(chrono::Utc::now())
    .bind(artist.label_id)
    .bind(Some(chrono::Utc::now()))
    .fetch_one(pool)
    .await?;

    let _release_artists = sqlx::query(
        "INSERT INTO release_artists (release_id, artist_id) VALUES ($1, $2) RETURNING *",
    )
    .bind(release.id)
    .bind(artist.id)
    .fetch_one(pool)
    .await?;

    Ok(release)
}

/// Create test track
///
/// # Arguments
/// * `pool` - The database connection pool
/// * `id` - The ID of the track
/// * `release` - The release the track is on (optional)
/// * `artist` - The artist the track is by (optional)
///
/// # Returns
/// The created track
///
/// # Errors
/// If the track cannot be created, return an error
///
/// # Panics
/// If the track cannot be created, panic
/// If the release is not found or cannot be created, panic
/// If the artist is not found or cannot be created, panic
#[cfg(feature = "ssr")]
pub async fn create_test_track(
    pool: &PgPool,
    id: usize,
    release: Option<Release>,
    artist: Option<Artist>,
) -> Result<Track, sqlx::Error> {
    let artist = match artist {
        Some(artist) => artist,
        None => {
            if release.is_some() {
                Artist::get_by_id(pool, release.clone().unwrap().primary_artist_id)
                    .await
                    .unwrap()
            } else {
                create_test_artist(pool, id, None).await.unwrap()
            }
        }
    };
    let release = match release {
        Some(release) => release,
        None => create_test_release(pool, id, Some(artist.clone()))
            .await
            .unwrap(),
    };

    let isrc_code = format!("UKXXX25{id:0>5}");
    let track = sqlx::query_as::<_, Track>("INSERT INTO tracks (name, slug, description, primary_artist_id, isrc_code, bpm, published_at) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *")
        .bind(format!("Test Track {id}"))
        .bind(format!("test-track-{id}"))
        .bind(format!("A track for testing purposes with the id of {id}"))
        .bind(release.primary_artist_id)
        .bind(isrc_code)
        .bind(120)
        .bind(Some(chrono::Utc::now()))
        .fetch_one(pool)
        .await?;

    let _track_artists =
        sqlx::query("INSERT INTO track_artists (track_id, artist_id) VALUES ($1, $2) RETURNING *")
            .bind(track.id)
            .bind(artist.id)
            .fetch_one(pool)
            .await?;

    let _track_releases = sqlx::query(
        "INSERT INTO release_tracks (release_id, track_id) VALUES ($1, $2) RETURNING *",
    )
    .bind(release.id)
    .bind(track.id)
    .fetch_one(pool)
    .await?;

    Ok(track)
}

/// Create test user
///
/// # Arguments
/// * `pool` - The database connection pool
/// * `id` - The ID of the user
///
/// # Returns
/// The created user
///
/// # Errors
/// If the user cannot be created, return an error
///
/// # Panics
/// If the user cannot be created, panic
#[cfg(feature = "ssr")]
pub async fn create_test_user(pool: &PgPool, id: usize) -> Result<SqlUser, sqlx::Error> {
    let user = sqlx::query_as::<_, SqlUser>(
        "INSERT INTO users (username, email, password) VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(format!("test-{id}"))
    .bind(format!("test-{id}@example.com"))
    .bind("$2b$12$bvHwxi3jnJC6/nzyFmKKBOZPHo/kn5KHPKxTeG0wiGOUlKuYYjZH.") // This is a valid bcrypt hash for the word "password"
    .fetch_one(pool)
    .await?;

    Ok(user)
}

/// Create test user with provided permissions
///
/// # Arguments
/// * `pool` - The database connection pool
/// * `id` - The ID of the user
/// * `permissions` - The permissions to assign to the user
///
/// # Returns
/// The created user with the provided permissions
///
/// # Errors
/// If the user cannot be created, return an error
/// If the permissions cannot be assigned to the user, return an error
///
/// # Panics
/// If the user cannot be created, panic
#[cfg(feature = "ssr")]
pub async fn create_test_user_with_permissions(
    pool: &PgPool,
    id: usize,
    permissions: Vec<&str>,
) -> Result<User, sqlx::Error> {
    let user = create_test_user(pool, id).await.unwrap();

    let mut permission_tokens = vec![];
    for permission in permissions.clone() {
        let token = sqlx::query_as::<_, SqlPermissionTokens>(
            "INSERT INTO user_permissions (user_id, token) VALUES ($1, $2) RETURNING *",
        )
        .bind(user.id)
        .bind(permission)
        .fetch_one(pool)
        .await?;
        permission_tokens.push(token);
    }

    let (user, _) = user.into_user(Some(permission_tokens));
    Ok(user)
}
