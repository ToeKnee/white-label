//! Track model
//!
//! The Track struct is used to represent a record track in the database.

use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use sqlx::{FromRow, PgPool};

use super::traits::Validate;
#[cfg(feature = "ssr")]
use super::{artist::Artist, release::Release};
#[cfg(feature = "ssr")]
use crate::utils::slugify::slugify;

/// The Track struct is used to represent a record track in the database.
#[derive(Serialize, Deserialize, Clone, Default, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "ssr", derive(FromRow))]
pub struct Track {
    /// The unique identifier of the track
    pub id: i64,
    /// The name of the track
    pub name: String,
    /// The slug of the track
    pub slug: String,
    /// The description of the track
    pub description: String,
    /// The primary artist
    /// This can also be included in the artsts relation, but it must contain one artist.
    /// Other artists are considered contributing artists
    pub primary_artist_id: i64,
    /// The primary image of the track
    pub primary_image: Option<String>,
    /// The ISRC code of the track
    /// This is a unique identifier for the track (changes for different versions/masters etc.)
    pub isrc_code: Option<String>,
    /// The BPM or beats per minute of the track
    /// For tracks with variable BPM, this value is undefined
    pub bpm: Option<i32>,
    /// The date the track is published.
    /// If this is None, the track is not published
    /// If this is in the future, the track is scheduled to be published
    /// If this is in the past, the track is published
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
    /// The date and time the track was created in the database
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// The date and time the track was last updated
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// The date and time the track was deleted
    /// If this is None, the track is not deleted
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl Validate for Track {
    #[cfg(feature = "ssr")]
    async fn validate(&self, pool: &PgPool) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow::anyhow!("Name is required."));
        }
        if self.name.len() > 255 {
            return Err(anyhow::anyhow!(
                "Name must be less than 255 characters.".to_string()
            ));
        }

        if self.slug.len() > 255 {
            return Err(anyhow::anyhow!(
                "Slug must be less than 255 characters.".to_string()
            ));
        }
        // Check that the slug is unique
        if let Ok(track) = Self::get_by_slug(pool, self.slug.clone()).await {
            if track.id != self.id {
                return Err(anyhow::anyhow!("Slug must be unique.".to_string()));
            }
        }

        // Check that the artist referenced in the primary_artist_id exists
        if let Err(e) = Artist::get_by_id(pool, self.primary_artist_id).await {
            tracing::error!("{e}");
            return Err(anyhow::anyhow!(
                "Artist with id {} does not exist.",
                self.primary_artist_id
            ));
        }

        if let Some(ref isrc_code) = self.isrc_code {
            if isrc_code.len() != 12 {
                return Err(anyhow::anyhow!(
                    "ISRC code must be 12 characters.".to_string()
                ));
            }
            // Check that the catalogue number is unique to the record label
            let row = sqlx::query("SELECT * FROM tracks WHERE isrc_code = $1 AND id != $2")
                .bind(isrc_code)
                .bind(self.id)
                .fetch_one(pool)
                .await;
            if row.is_ok() {
                return Err(anyhow::anyhow!("ISRC code must be unique.".to_string()));
            }
        }

        Ok(())
    }
}

impl Track {
    /// Get the primary image URL
    /// If the primary image is None, return the default image
    pub fn primary_image_url(&self) -> String {
        self.primary_image.clone().map_or_else(
            || "/Logo.svg".to_string(),
            |file| format!("/uploads/tracks/{file}"),
        )
    }

    /// Create a new track
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `name` - The name of the track
    /// * `description` - The description of the track
    /// * `isrc_code` - The ISRC code of the track
    /// * `bpm` - The BPM of the track
    ///
    /// # Returns
    /// The created track
    ///
    /// # Errors
    /// If the track cannot be created, return an error
    /// If the record label is not found, return an error
    #[allow(clippy::too_many_arguments)]
    #[cfg(feature = "ssr")]
    pub async fn create(
        pool: &PgPool,
        name: String,
        description: String,
        primary_artist_id: i64,
        isrc_code: Option<String>,
        bpm: Option<i32>,
        published_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> anyhow::Result<Self> {
        let slug = slugify(&name);

        let track = Self {
            id: 0,
            name,
            slug,
            description,
            primary_artist_id,
            primary_image: None,
            isrc_code,
            bpm,
            published_at,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };
        track.validate(pool).await?;

        let track = sqlx::query_as::<_, Self>(
         "INSERT INTO tracks (name, slug, description, primary_artist_id, isrc_code, bpm,  published_at) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *",
     )
         .bind(track.name)
         .bind(track.slug)
         .bind(track.description)
         .bind(track.primary_artist_id)
         .bind(track.isrc_code)
         .bind(track.bpm)
         .bind(track.published_at)
         .fetch_one(pool)
         .await?;

        Ok(track)
    }

    /// Get track by slug
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `slug` - The slug of the track
    ///
    /// # Returns
    /// The track
    ///
    /// # Errors
    /// If the track cannot be found, return an error
    #[cfg(feature = "ssr")]
    pub async fn get_by_slug(pool: &PgPool, slug: String) -> anyhow::Result<Self> {
        let track = sqlx::query_as::<_, Self>("SELECT * FROM tracks WHERE slug = $1")
            .bind(slug.clone())
            .fetch_one(pool)
            .await;

        match track {
            Ok(track) => Ok(track),
            Err(e) => {
                tracing::error!("{e}");
                Err(anyhow::anyhow!("Could not find track with slug {slug}."))
            }
        }
    }

    /// Get specific track by artist and record label and slug
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `release_id` - The ID of the release
    /// * `artist_id` - The ID of the artist
    /// * `record_label_id` - The ID of the record label
    /// * `slug` - The slug of the track
    /// * `include_hidden` - Whether to include untrackd tracks
    ///
    /// # Returns
    /// The track
    ///
    /// # Errors
    /// If there is an error getting the tracks, return an error
    #[cfg(feature = "ssr")]
    pub async fn get_by_release_and_artist_and_record_label_and_slug(
        pool: &PgPool,
        release_id: i64,
        artist_id: i64,
        record_label_id: i64,
        slug: String,
        include_hidden: bool,
    ) -> anyhow::Result<Self> {
        let query = if include_hidden {
            "SELECT t.*
             FROM tracks t
             INNER JOIN release_tracks rt
             ON t.id = rt.track_id
             INNER JOIN release_artists ra
             ON rt.release_id = ra.release_id
             INNER JOIN artists a
             ON a.id = ra.artist_id
             WHERE t.slug = $1
                AND rt.release_id = $2
                AND a.id = $3
                AND a.label_id = $4"
        } else {
            "SELECT t.*
             FROM tracks t
             INNER JOIN release_tracks rt
             ON t.id = rt.track_id
             INNER JOIN release_artists ra
             ON rt.release_id = ra.release_id
             INNER JOIN artists a
             ON a.id = ra.artist_id
             WHERE t.slug = $1
                AND rt.release_id = $2
                AND a.id = $3
                AND a.label_id = $4
                AND t.deleted_at IS NULL
                AND t.published_at < NOW()
                AND t.published_at IS NOT NULL"
        };

        let track = sqlx::query_as::<_, Self>(query)
            .bind(slug.clone())
            .bind(release_id)
            .bind(artist_id)
            .bind(record_label_id)
            .bind(slug.clone())
            .fetch_one(pool)
            .await;

        match track {
            Ok(track) => Ok(track),
            Err(e) => {
                tracing::error!("{e}");
                Err(anyhow::anyhow!(
                    "Could not find track {} for release with id {} and artist with id {} and record label with id {}.",
                    slug,
                    release_id,
                    artist_id,
                    record_label_id,
                ))
            }
        }
    }

    /// List tracks by artist and record label
    /// This is used to get all tracks by an artist on a record label
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `release_id` - The ID of the relase
    /// * `artist_id` - The ID of the artist
    /// * `record_label_id` - The ID of the record label
    /// * `include_hidden` - Whether to include untrackd tracks
    ///
    /// # Returns
    /// The tracks
    ///
    /// # Errors
    /// If there is an error getting the tracks, return an error
    #[cfg(feature = "ssr")]
    pub async fn list_by_release_and_artist_and_record_label(
        pool: &PgPool,
        release_id: i64,
        artist_id: i64,
        record_label_id: i64,
        include_hidden: bool,
    ) -> anyhow::Result<Vec<Self>> {
        let query = if include_hidden {
            "SELECT t.*
             FROM tracks t
             INNER JOIN release_tracks rt
             ON t.id = rt.track_id
             INNER JOIN release_artists ra
             ON rt.release_id = ra.release_id
             INNER JOIN artists a
             ON a.id = ra.artist_id
             WHERE rt.release_id = $1 AND a.id = $2 AND a.label_id = $3
             ORDER BY t.deleted_at DESC, t.name ASC"
        } else {
            "SELECT t.*
             FROM tracks t
             INNER JOIN release_tracks rt
             ON t.id = rt.track_id
             INNER JOIN release_artists ra
             ON rt.release_id = ra.release_id
             INNER JOIN artists a
             ON a.id = ra.artist_id
             WHERE rt.release_id = $1 AND a.id = $2 AND a.label_id = $3
              AND t.deleted_at IS NULL
              AND t.published_at < NOW()
              AND t.published_at IS NOT NULL
             ORDER BY t.name ASC"
        };

        let tracks = sqlx::query_as::<_, Self>(query)
            .bind(release_id)
            .bind(artist_id)
            .bind(record_label_id)
            .fetch_all(pool)
            .await;

        match tracks {
            Ok(tracks) => Ok(tracks),
            Err(e) => {
                tracing::error!("{e}");
                Err(anyhow::anyhow!(
                    "Could not find tracks for release with id {} and artist with id {} and record label with id {}.",
                    release_id,
                    artist_id,
                    record_label_id
                ))
            }
        }
    }

    /// Update an track
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    ///
    /// # Returns
    /// The updated track
    ///
    /// # Errors
    /// If the track cannot be updated, return an error
    ///
    /// # Panics
    /// If the track cannot be updated, return an error
    #[cfg(feature = "ssr")]
    pub async fn update(mut self, pool: &PgPool) -> anyhow::Result<Self> {
        self.slug = slugify(&self.name);
        self.validate(pool).await?;

        let track = match sqlx::query_as::<_, Self>(
            "UPDATE tracks SET name = $1, slug = $2, description = $3, primary_artist_id = $4, primary_image = $5, isrc_code = $6, bpm = $7, published_at = $8, updated_at = $9 WHERE id = $10 RETURNING *",
        )
        .bind(self.name)
        .bind(self.slug)
        .bind(self.description)
        .bind(self.primary_artist_id)
        .bind(self.primary_image)
        .bind(self.isrc_code)
        .bind(self.bpm)
        .bind(self.published_at)
        .bind(chrono::Utc::now())
        .bind(self.id)
        .fetch_one(pool)
        .await {
            Ok(track) => track,
            Err(e) => {
                tracing::error!("{e}");
                return Err(anyhow::anyhow!(
                    "Could not update track with id {}. {e}",
                    self.id
                ));
            }
        };

        Ok(track)
    }

    /// Delete an track
    /// This is a soft delete
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    ///
    /// # Returns
    /// The deleted track
    ///
    /// # Errors
    /// If the track cannot be deleted, return an error
    #[cfg(feature = "ssr")]
    pub async fn delete(&self, pool: &PgPool) -> anyhow::Result<Self> {
        let track = sqlx::query_as::<_, Self>(
            "UPDATE tracks SET deleted_at = $1 WHERE id = $2 RETURNING *",
        )
        .bind(chrono::Utc::now())
        .bind(self.id)
        .fetch_one(pool)
        .await;

        match track {
            Ok(track) => Ok(track),
            Err(e) => {
                tracing::error!("{e}");
                Err(anyhow::anyhow!(
                    "Could not delete track with id {}.",
                    self.id
                ))
            }
        }
    }

    /// Set the artists for the track
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `artist_ids` - The IDs of the artists
    /// # Returns
    /// The track
    /// # Errors
    /// If the track cannot be updated, return an error
    /// # Panics
    /// If the track cannot be updated, return an error
    #[cfg(feature = "ssr")]
    pub async fn set_artists(&self, pool: &PgPool, artist_ids: Vec<i64>) -> anyhow::Result<Self> {
        if artist_ids.is_empty() {
            return Err(anyhow::anyhow!("Artist IDs cannot be empty."));
        }

        let mut tx = pool.begin().await?;

        // Delete all artists for the track
        sqlx::query("DELETE FROM track_artists WHERE track_id = $1")
            .bind(self.id)
            .execute(&mut *tx)
            .await?;

        // Insert the new artists
        for artist_id in artist_ids {
            match sqlx::query("INSERT INTO track_artists (track_id, artist_id) VALUES ($1, $2)")
                .bind(self.id)
                .bind(artist_id)
                .execute(&mut *tx)
                .await
            {
                Ok(_) => (),
                Err(e) => {
                    tracing::error!("{e}");
                    return Err(anyhow::anyhow!(
                        "Could not set artists for track with id {}.",
                        self.id
                    ));
                }
            }
        }

        let _ = tx.commit().await;

        Ok(self.clone())
    }

    /// Get the artists for the track
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    ///
    /// # Returns
    /// The artists for the track
    ///
    /// # Errors
    /// If the track cannot be found, return an error
    /// # Panics
    /// If the track cannot be found, return an error
    #[cfg(feature = "ssr")]
    pub async fn get_artists(&self, pool: &PgPool) -> anyhow::Result<Vec<Artist>> {
        let artists = sqlx::query_as::<_, Artist>(
            "SELECT artists.* FROM artists
             INNER JOIN track_artists ON artists.id = track_artists.artist_id
             WHERE track_artists.track_id = $1",
        )
        .bind(self.id)
        .fetch_all(pool)
        .await?;

        Ok(artists)
    }

    /// Set the releases for the track
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `release_ids` - The IDs of the releases
    /// # Returns
    /// The track
    /// # Errors
    /// If the track cannot be updated, return an error
    /// # Panics
    /// If the track cannot be updated, return an error
    #[cfg(feature = "ssr")]
    pub async fn set_releases(&self, pool: &PgPool, release_ids: Vec<i64>) -> anyhow::Result<Self> {
        if release_ids.is_empty() {
            return Err(anyhow::anyhow!("release IDs cannot be empty."));
        }

        let mut tx = pool.begin().await?;

        // Delete all releases for the track
        sqlx::query("DELETE FROM release_tracks WHERE track_id = $1")
            .bind(self.id)
            .execute(&mut *tx)
            .await?;

        // Insert the new releases
        for release_id in release_ids {
            match sqlx::query("INSERT INTO release_tracks (release_id, track_id) VALUES ($1, $2)")
                .bind(release_id)
                .bind(self.id)
                .execute(&mut *tx)
                .await
            {
                Ok(_) => (),
                Err(e) => {
                    tracing::error!("{e}");
                    return Err(anyhow::anyhow!(
                        "Could not set releases for track with id {}.",
                        self.id
                    ));
                }
            }
        }

        let _ = tx.commit().await;

        Ok(self.clone())
    }

    /// Get the releases for the track
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    ///
    /// # Returns
    /// The releases for the track
    ///
    /// # Errors
    /// If the track cannot be found, return an error
    /// # Panics
    /// If the track cannot be found, return an error
    #[cfg(feature = "ssr")]
    pub async fn get_releases(&self, pool: &PgPool) -> anyhow::Result<Vec<Release>> {
        let releases = sqlx::query_as::<_, Release>(
            "SELECT releases.* FROM releases
             INNER JOIN release_tracks ON releases.id = release_tracks.release_id
             WHERE release_tracks.track_id = $1",
        )
        .bind(self.id)
        .fetch_all(pool)
        .await?;

        Ok(releases)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "ssr")]
    use crate::models::record_label::RecordLabel;
    #[cfg(feature = "ssr")]
    use crate::models::test_helpers::{
        create_test_artist, create_test_record_label, create_test_release, create_test_track,
    };

    #[sqlx::test]
    async fn test_validate_success(pool: PgPool) {
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();
        let track = Track {
            id: 1,
            name: "Test Track".to_string(),
            slug: "test-track".to_string(),
            description: "This is a test track".to_string(),
            primary_artist_id: artist.id,
            primary_image: None,
            isrc_code: Some("UKUXX2020123".to_string()),
            bpm: Some(120),
            published_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let result = track.validate(&pool).await;

        assert!(result.is_ok());
    }

    #[sqlx::test]
    async fn test_validate_name_is_empty(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let track = Track {
            id: 1,
            name: String::new(),
            slug: "test-track".to_string(),
            description: "This is a test track".to_string(),
            primary_artist_id: artist.id,
            primary_image: None,
            isrc_code: Some("UKXXX2020123".to_string()),
            bpm: Some(120),
            published_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let result = track.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Name is required.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_validate_name_length(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let name = "a".repeat(256);
        let track = Track {
            id: 1,
            name,
            slug: "test-track".to_string(),
            description: "This is a test track".to_string(),
            primary_artist_id: artist.id,
            primary_image: None,
            isrc_code: Some("UKXXX2020123".to_string()),
            bpm: Some(120),
            published_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let result = track.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Name must be less than 255 characters.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_validate_slug_length(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let slug = "a".repeat(256);
        let track = Track {
            id: 1,
            name: "Test Track".to_string(),
            slug,
            description: "This is a test track".to_string(),
            primary_artist_id: artist.id,
            primary_image: None,
            isrc_code: Some("UKXXX2020123".to_string()),
            bpm: Some(120),
            published_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let result = track.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Slug must be less than 255 characters.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_validate_slug_unique(pool: PgPool) {
        let track = create_test_track(&pool, 1, None, None).await.unwrap();
        let mut new_track = track.clone();
        new_track.id = 2;
        new_track.slug = track.slug.clone();

        let result = new_track.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Slug must be unique.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_primary_artist_does_not_exist(pool: PgPool) {
        let track = Track {
            id: 1,
            name: "Test Track".to_string(),
            slug: "test-track".to_string(),
            description: "This is a test track".to_string(),
            primary_artist_id: 1,
            primary_image: None,
            isrc_code: Some("UKXXX2020123".to_string()),
            bpm: Some(120),
            published_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let result = track.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Artist with id 1 does not exist.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_validate_isrc_code_length(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();

        let isrc_code = "a".repeat(13);
        let track = Track {
            id: 1,
            name: "Test Track".to_string(),
            slug: "test-track".to_string(),
            description: "This is a test track".to_string(),
            primary_artist_id: artist.id,
            primary_image: None,
            isrc_code: Some(isrc_code),
            bpm: Some(123),
            published_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let result = track.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "ISRC code must be 12 characters.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_validate_isrc_code_unique(pool: PgPool) {
        let track = create_test_track(&pool, 1, None, None).await.unwrap();
        let mut new_track = track.clone();
        new_track.id = 2;
        new_track.slug = "new-track-2".to_string();
        new_track.isrc_code = track.isrc_code;

        let result = new_track.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "ISRC code must be unique.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_create(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let track = Track::create(
            &pool,
            "Test Track".to_string(),
            "This is a test track".to_string(),
            artist.id,
            Some("UKXXX2020123".to_string()),
            Some(120),
            Some(chrono::Utc::now()),
        )
        .await
        .unwrap();

        assert_eq!(track.name, "Test Track".to_string());
        assert_eq!(track.description, "This is a test track".to_string());
    }

    #[sqlx::test]
    async fn test_create_with_validation_error(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let track = Track::create(
            &pool,
            String::new(),
            "This is a test track".to_string(),
            artist.id,
            Some("UKXXX2020123".to_string()),
            Some(120),
            Some(chrono::Utc::now()),
        )
        .await;

        assert!(track.is_err());
        assert_eq!(
            track.unwrap_err().to_string(),
            "Name is required.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_get_by_slug(pool: PgPool) {
        let track = create_test_track(&pool, 1, None, None).await.unwrap();
        let track_by_slug = Track::get_by_slug(&pool, track.slug.clone()).await.unwrap();

        assert_eq!(track, track_by_slug);
    }

    #[sqlx::test]
    async fn test_get_by_slug_not_found(pool: PgPool) {
        let track = Track::get_by_slug(&pool, "missing".to_string()).await;

        assert!(track.is_err());
        assert_eq!(
            track.unwrap_err().to_string(),
            "Could not find track with slug missing.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_get_by_release_and_artist_and_record_label_and_slug_no_tracks(pool: PgPool) {
        let track = Track::get_by_release_and_artist_and_record_label_and_slug(
            &pool,
            1,
            1,
            1,
            "test".to_string(),
            true,
        )
        .await;

        assert!(track.is_err());
        assert_eq!(
            track.unwrap_err().to_string(),
            "Could not find track test for release with id 1 and artist with id 1 and record label with id 1.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_get_by_release_and_artist_and_record_label_and_slug(pool: PgPool) {
        let test_track = create_test_track(&pool, 1, None, None).await.unwrap();
        let track = Track::get_by_release_and_artist_and_record_label_and_slug(
            &pool,
            1,
            1,
            1,
            test_track.slug.clone(),
            true,
        )
        .await
        .unwrap();

        assert_eq!(track.id, test_track.id);
        assert_eq!(track.slug, test_track.slug);
    }

    #[sqlx::test]
    async fn test_get_by_release_and_artist_and_record_label_and_slug_deleted_track_include_hidden(
        pool: PgPool,
    ) {
        let deleted_track = create_test_track(&pool, 1, None, None).await.unwrap();
        deleted_track.delete(&pool).await.unwrap();
        let track = Track::get_by_release_and_artist_and_record_label_and_slug(
            &pool,
            1,
            1,
            1,
            deleted_track.slug.clone(),
            true,
        )
        .await
        .unwrap();

        assert_eq!(track.id, deleted_track.id);
    }

    #[sqlx::test]
    async fn test_get_by_release_and_artist_and_record_label_and_slug_deleted_track(pool: PgPool) {
        let deleted_track = create_test_track(&pool, 1, None, None).await.unwrap();
        deleted_track.delete(&pool).await.unwrap();
        let track = Track::get_by_release_and_artist_and_record_label_and_slug(
            &pool,
            1,
            1,
            1,
            deleted_track.slug.clone(),
            false,
        )
        .await;

        assert!(track.is_err());
        assert_eq!(
            track.unwrap_err().to_string(),
            "Could not find track test-track-1 for release with id 1 and artist with id 1 and record label with id 1.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_get_by_release_and_artist_and_record_label_and_slug_unpublished_track_include_hidden(
        pool: PgPool,
    ) {
        let mut unpublished_track = create_test_track(&pool, 1, None, None).await.unwrap();
        unpublished_track.published_at = None;
        unpublished_track.clone().update(&pool).await.unwrap();
        let track = Track::get_by_release_and_artist_and_record_label_and_slug(
            &pool,
            1,
            1,
            1,
            unpublished_track.slug.clone(),
            true,
        )
        .await
        .unwrap();

        assert_eq!(track.id, unpublished_track.id);
        assert_eq!(track.slug, unpublished_track.slug);
    }

    #[sqlx::test]
    async fn test_get_by_release_and_artist_and_record_label_and_slug_unpublished_track(
        pool: PgPool,
    ) {
        let mut unpublished_track = create_test_track(&pool, 1, None, None).await.unwrap();
        unpublished_track.published_at = None;
        unpublished_track.clone().update(&pool).await.unwrap();
        let track = Track::get_by_release_and_artist_and_record_label_and_slug(
            &pool,
            1,
            1,
            1,
            unpublished_track.slug.clone(),
            false,
        )
        .await;

        assert!(track.is_err());
        assert_eq!(
            track.unwrap_err().to_string(),
            "Could not find track test-track-1 for release with id 1 and artist with id 1 and record label with id 1.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_get_by_release_and_artist_and_record_label_and_slug_future_track_include_hidden(
        pool: PgPool,
    ) {
        let mut future_track = create_test_track(&pool, 1, None, None).await.unwrap();
        future_track.published_at = Some(chrono::Utc::now() + chrono::Duration::days(1));
        future_track.clone().update(&pool).await.unwrap();
        let track = Track::get_by_release_and_artist_and_record_label_and_slug(
            &pool,
            1,
            1,
            1,
            future_track.slug.clone(),
            true,
        )
        .await
        .unwrap();

        assert_eq!(track.id, future_track.id);
        assert_eq!(track.slug, future_track.slug);
    }

    #[sqlx::test]
    async fn test_get_by_release_and_artist_and_record_label_and_slug_future_track(pool: PgPool) {
        let mut future_track = create_test_track(&pool, 1, None, None).await.unwrap();
        future_track.published_at = Some(chrono::Utc::now() + chrono::Duration::days(1));
        future_track.clone().update(&pool).await.unwrap();
        let track = Track::get_by_release_and_artist_and_record_label_and_slug(
            &pool,
            1,
            1,
            1,
            future_track.slug.clone(),
            false,
        )
        .await;

        assert!(track.is_err());
        assert_eq!(
            track.unwrap_err().to_string(),
            "Could not find track test-track-1 for release with id 1 and artist with id 1 and record label with id 1.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_list_by_release_and_artist_and_record_label_no_tracks(pool: PgPool) {
        let tracks = Track::list_by_release_and_artist_and_record_label(&pool, 1, 1, 1, true)
            .await
            .unwrap();

        assert_eq!(tracks.len(), 0);
    }

    #[sqlx::test]
    async fn test_list_by_release_and_artist_and_record_label_with_tracks(pool: PgPool) {
        let track = create_test_track(&pool, 1, None, None).await.unwrap();
        let tracks = Track::list_by_release_and_artist_and_record_label(&pool, 1, 1, 1, true)
            .await
            .unwrap();

        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, track.id);
    }

    #[sqlx::test]
    async fn test_list_by_release_and_artist_and_record_label_with_unpublished_tracks(
        pool: PgPool,
    ) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let record_label_id = artist.label_id;
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();
        let mut unpublished_track =
            create_test_track(&pool, 1, Some(release.clone()), Some(artist.clone()))
                .await
                .unwrap();
        unpublished_track.published_at = None;
        unpublished_track.update(&pool).await.unwrap();
        let mut published_in_future_track =
            create_test_track(&pool, 2, Some(release.clone()), Some(artist.clone()))
                .await
                .unwrap();
        published_in_future_track.published_at =
            Some(chrono::Utc::now() + chrono::Duration::days(1));
        published_in_future_track.update(&pool).await.unwrap();
        let mut published_track =
            create_test_track(&pool, 3, Some(release.clone()), Some(artist.clone()))
                .await
                .unwrap();
        published_track.published_at = Some(chrono::Utc::now() - chrono::Duration::days(1));
        published_track.clone().update(&pool).await.unwrap();
        let deleted_track =
            create_test_track(&pool, 4, Some(release.clone()), Some(artist.clone()))
                .await
                .unwrap();
        deleted_track.delete(&pool).await.unwrap();

        let tracks = Track::list_by_release_and_artist_and_record_label(
            &pool,
            release.id,
            artist.id,
            record_label_id,
            false,
        )
        .await
        .unwrap();

        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, published_track.id);
    }

    #[sqlx::test]
    async fn test_list_by_release_and_artist_and_record_label_with_unpublished_tracks_show_all(
        pool: PgPool,
    ) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let record_label_id = artist.label_id;
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();

        let mut unpublished_track =
            create_test_track(&pool, 1, Some(release.clone()), Some(artist.clone()))
                .await
                .unwrap();
        unpublished_track.published_at = None;
        unpublished_track.clone().update(&pool).await.unwrap();
        let mut published_in_future_track =
            create_test_track(&pool, 2, Some(release.clone()), Some(artist.clone()))
                .await
                .unwrap();
        published_in_future_track.published_at =
            Some(chrono::Utc::now() + chrono::Duration::days(1));
        published_in_future_track
            .clone()
            .update(&pool)
            .await
            .unwrap();
        let mut published_track =
            create_test_track(&pool, 3, Some(release.clone()), Some(artist.clone()))
                .await
                .unwrap();
        published_track.published_at = Some(chrono::Utc::now() - chrono::Duration::days(1));
        published_track.clone().update(&pool).await.unwrap();
        let deleted_track =
            create_test_track(&pool, 4, Some(release.clone()), Some(artist.clone()))
                .await
                .unwrap();
        deleted_track.clone().delete(&pool).await.unwrap();

        let tracks = Track::list_by_release_and_artist_and_record_label(
            &pool,
            release.id,
            artist.id,
            record_label_id,
            true,
        )
        .await
        .unwrap();

        assert_eq!(tracks.len(), 4);
        assert_eq!(tracks[0].id, unpublished_track.id);
        assert_eq!(tracks[1].id, published_in_future_track.id);
        assert_eq!(tracks[2].id, published_track.id);
        assert_eq!(tracks[3].id, deleted_track.id);
    }

    #[sqlx::test]
    async fn test_list_by_release_and_artist_and_record_label_wrong_release(pool: PgPool) {
        create_test_track(&pool, 1, None, None).await.unwrap();
        let tracks = Track::list_by_release_and_artist_and_record_label(&pool, 2, 1, 1, true)
            .await
            .unwrap();

        assert_eq!(tracks.len(), 0);
    }

    #[sqlx::test]
    async fn test_list_by_release_and_artist_and_record_label_wrong_artist(pool: PgPool) {
        create_test_track(&pool, 1, None, None).await.unwrap();
        let tracks = Track::list_by_release_and_artist_and_record_label(&pool, 1, 2, 1, true)
            .await
            .unwrap();

        assert_eq!(tracks.len(), 0);
    }

    #[sqlx::test]
    async fn test_list_by_release_and_artist_and_record_label_wrong_label(pool: PgPool) {
        create_test_track(&pool, 1, None, None).await.unwrap();
        let tracks = Track::list_by_release_and_artist_and_record_label(&pool, 1, 1, 2, true)
            .await
            .unwrap();

        assert_eq!(tracks.len(), 0);
    }

    #[sqlx::test]
    async fn test_update(pool: PgPool) {
        let track = create_test_track(&pool, 1, None, None).await.unwrap();
        let mut update_track = track.clone();
        update_track.name = "Updated Track".to_string();
        update_track.description = "This is an updated track".to_string();
        update_track.primary_image = Some("an-image.jpg".to_string());
        update_track.isrc_code = Some("UKUTK2025321".to_string());

        let updated_track = update_track.update(&pool).await.unwrap();
        assert_eq!(updated_track.name, "Updated Track".to_string());
        assert_eq!(updated_track.slug, "updated-track".to_string());
        assert_eq!(
            updated_track.description,
            "This is an updated track".to_string()
        );
        assert_eq!(
            updated_track.primary_image,
            Some("an-image.jpg".to_string())
        );
        assert_eq!(updated_track.isrc_code, Some("UKUTK2025321".to_string()));
        assert_ne!(updated_track.updated_at, track.updated_at);
    }

    #[sqlx::test]
    async fn test_update_validation_error(pool: PgPool) {
        let track = create_test_track(&pool, 1, None, None).await.unwrap();
        let mut update_track = track.clone();
        update_track.name = String::new();
        let updated_track = update_track.update(&pool).await;

        assert!(updated_track.is_err());
        assert_eq!(
            updated_track.unwrap_err().to_string(),
            "Name is required.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_delete(pool: PgPool) {
        let track = create_test_track(&pool, 1, None, None).await.unwrap();
        let result = track.delete(&pool).await.unwrap();
        assert!(result.deleted_at.is_some());
    }

    #[sqlx::test]
    async fn test_delete_not_found(pool: PgPool) {
        let track = Track::default();
        let result = track.delete(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Could not delete track with id 0.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_set_artists(pool: PgPool) {
        let track = create_test_track(&pool, 1, None, None).await.unwrap();
        let artist = track.get_artists(&pool).await.unwrap()[0].clone();
        let record_label = RecordLabel::get_by_id(&pool, artist.label_id)
            .await
            .unwrap();
        let artist2 = create_test_artist(&pool, 2, Some(record_label))
            .await
            .unwrap();

        let result = track.set_artists(&pool, vec![artist.id, artist2.id]).await;

        assert!(result.is_ok());
        let artists = track.get_artists(&pool).await.unwrap();
        assert_eq!(artists.len(), 2);
        assert_eq!(artists[0].id, artist.id);
        assert_eq!(artists[1].id, artist2.id);
    }

    #[sqlx::test]
    async fn test_set_artists_not_found(pool: PgPool) {
        let track = Track::default();
        let result = track.set_artists(&pool, vec![1, 2]).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Could not set artists for track with id 0.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_set_artists_no_artists(pool: PgPool) {
        let track = create_test_track(&pool, 1, None, None).await.unwrap();
        let result = track.set_artists(&pool, vec![]).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Artist IDs cannot be empty.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_set_artists_replace(pool: PgPool) {
        let track = create_test_track(&pool, 1, None, None).await.unwrap();
        let artist = track.get_artists(&pool).await.unwrap()[0].clone();
        let record_label = RecordLabel::get_by_id(&pool, artist.label_id)
            .await
            .unwrap();
        let artist2 = create_test_artist(&pool, 2, Some(record_label.clone()))
            .await
            .unwrap();
        let artist3 = create_test_artist(&pool, 3, Some(record_label))
            .await
            .unwrap();

        // Set the artists for the track
        track
            .set_artists(&pool, vec![artist.id, artist2.id])
            .await
            .unwrap();

        // Replace the artists for the track
        let result = track.set_artists(&pool, vec![artist3.id]).await;

        assert!(result.is_ok());
        let artists = track.get_artists(&pool).await.unwrap();
        assert_eq!(artists.len(), 1);
        assert_eq!(artists[0].id, artist3.id);
    }

    /// Test get_artists
    #[sqlx::test]
    async fn test_get_artists(pool: PgPool) {
        let track = create_test_track(&pool, 1, None, None).await.unwrap();
        let artist = track.get_artists(&pool).await.unwrap()[0].clone();
        let record_label = RecordLabel::get_by_id(&pool, artist.label_id)
            .await
            .unwrap();
        let artist2 = create_test_artist(&pool, 2, Some(record_label))
            .await
            .unwrap();

        // Set the artists for the track
        track
            .set_artists(&pool, vec![artist.id, artist2.id])
            .await
            .unwrap();

        // Get the artists for the track
        let artists = track.get_artists(&pool).await.unwrap();

        assert_eq!(artists.len(), 2);
        assert_eq!(artists[0].id, artist.id);
        assert_eq!(artists[1].id, artist2.id);
    }

    #[sqlx::test]
    async fn test_set_releases(pool: PgPool) {
        let track = create_test_track(&pool, 1, None, None).await.unwrap();
        let release = track.get_releases(&pool).await.unwrap()[0].clone();
        let release2 = create_test_release(&pool, 2, None).await.unwrap();

        let result = track
            .set_releases(&pool, vec![release.id, release2.id])
            .await;

        assert!(result.is_ok());
        let releases = track.get_releases(&pool).await.unwrap();
        assert_eq!(releases.len(), 2);
        assert_eq!(releases[0].id, release.id);
        assert_eq!(releases[1].id, release2.id);
    }

    #[sqlx::test]
    async fn test_set_releases_not_found(pool: PgPool) {
        let track = Track::default();
        let result = track.set_releases(&pool, vec![1, 2]).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Could not set releases for track with id 0.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_set_releases_no_releases(pool: PgPool) {
        let track = create_test_track(&pool, 1, None, None).await.unwrap();
        let result = track.set_releases(&pool, vec![]).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "release IDs cannot be empty.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_set_releases_replace(pool: PgPool) {
        let track = create_test_track(&pool, 1, None, None).await.unwrap();
        let release = track.get_releases(&pool).await.unwrap()[0].clone();
        let release2 = create_test_release(&pool, 2, None).await.unwrap();
        let release3 = create_test_release(&pool, 3, None).await.unwrap();

        // Set the releases for the track
        track
            .set_releases(&pool, vec![release.id, release2.id])
            .await
            .unwrap();

        // Replace the releases for the track
        let result = track.set_releases(&pool, vec![release3.id]).await;

        assert!(result.is_ok());
        let releases = track.get_releases(&pool).await.unwrap();
        assert_eq!(releases.len(), 1);
        assert_eq!(releases[0].id, release3.id);
    }

    /// Test get_releases
    #[sqlx::test]
    async fn test_get_releases(pool: PgPool) {
        let track = create_test_track(&pool, 1, None, None).await.unwrap();
        let release = track.get_releases(&pool).await.unwrap()[0].clone();
        let release2 = create_test_release(&pool, 2, None).await.unwrap();

        // Set the releases for the track
        track
            .set_releases(&pool, vec![release.id, release2.id])
            .await
            .unwrap();

        // Get the releases for the track
        let releases = track.get_releases(&pool).await.unwrap();

        assert_eq!(releases.len(), 2);
        assert_eq!(releases[0].id, release.id);
        assert_eq!(releases[1].id, release2.id);
    }

    #[sqlx::test]
    async fn test_primary_image_url(pool: PgPool) {
        let track = create_test_track(&pool, 1, None, None).await.unwrap();
        let url = track.primary_image_url();
        assert_eq!(url, "/Logo.svg");
    }

    #[sqlx::test]
    async fn test_primary_image_url_with_custom_image(pool: PgPool) {
        let track = create_test_track(&pool, 1, None, None).await.unwrap();
        let mut track = track;
        track.primary_image = Some("custom-image.jpg".to_string());
        let url = track.primary_image_url();
        assert_eq!(url, "/uploads/tracks/custom-image.jpg");
    }
}
