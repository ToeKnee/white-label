//! Release model
//!
//! The Release struct is used to represent a record release in the database.

use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use sqlx::{FromRow, PgPool, Row};

use super::traits::Validate;
#[cfg(feature = "ssr")]
use super::{
    artist::Artist, record_label::RecordLabel, track::Track, track_with_artists::TrackWithArtists,
};
#[cfg(feature = "ssr")]
use crate::utils::slugify::slugify;

/// The Release struct is used to represent a record release in the database.
#[derive(Serialize, Deserialize, Clone, Default, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "ssr", derive(FromRow))]
pub struct Release {
    /// The unique identifier of the release
    pub id: i64,
    /// The name of the release
    pub name: String,
    /// The slug of the release
    pub slug: String,
    /// The description of the release
    pub description: String,
    /// The primary artist
    /// This can also be included in the artsts relation, but it must contain one artist.
    /// Other artists are considered contributing artists
    pub primary_artist_id: i64,
    /// The primary image of the release
    pub primary_image: Option<String>,
    /// The catalogue number of the release
    /// This is unique to the record label
    pub catalogue_number: String,
    /// The release date of the release
    /// This is the date the release is available to the public
    /// If this is None, the release is not released
    /// If this is in the future, the release is scheduled to be released
    /// If this is in the past, the release is released
    /// This is not the same as the `published_at` date
    /// The `published_at` date is the date the release is made public
    /// The `release_date` is the date the release is available to the public
    pub release_date: Option<chrono::DateTime<chrono::Utc>>,
    /// The label id
    pub label_id: i64,
    /// The date the release is published.
    /// If this is None, the release is not published
    /// If this is in the future, the release is scheduled to be published
    /// If this is in the past, the release is published
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
    /// The date and time the release was created in the database
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// The date and time the release was last updated
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// The date and time the release was deleted
    /// If this is None, the release is not deleted
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl Validate for Release {
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
        if let Ok(release) = Self::get_by_slug(pool, self.slug.clone()).await
            && release.id != self.id
        {
            return Err(anyhow::anyhow!("Slug must be unique.".to_string()));
        }

        // Check that the artist referenced in the primary_artist_id exists
        if let Err(e) = Artist::get_by_id(pool, self.primary_artist_id).await {
            tracing::error!("{e}");
            return Err(anyhow::anyhow!(
                "Artist with id {} does not exist.",
                self.primary_artist_id
            ));
        }

        if self.catalogue_number.len() > 255 {
            return Err(anyhow::anyhow!(
                "Catalogue number must be less than 255 characters.".to_string()
            ));
        }
        // Check that the catalogue number is unique to the record label
        let row = sqlx::query(
            "SELECT * FROM releases WHERE catalogue_number = $1 AND label_id = $2 AND id != $3",
        )
        .bind(self.catalogue_number.clone())
        .bind(self.label_id)
        .bind(self.id)
        .fetch_one(pool)
        .await;
        if row.is_ok() {
            return Err(anyhow::anyhow!(
                "Catalogue number must be unique.".to_string()
            ));
        }

        // Check that the record label exists
        if let Err(e) = RecordLabel::get_by_id(pool, self.label_id).await {
            tracing::error!("{e}");
            return Err(anyhow::anyhow!(
                "Record Label with id {} does not exist.",
                self.label_id
            ));
        }

        Ok(())
    }
}

impl Release {
    /// Get the primary image URL
    /// If the primary image is None, return the default image
    pub fn primary_image_url(&self) -> String {
        self.primary_image.clone().map_or_else(
            || "/Logo.svg".to_string(),
            |file| format!("/uploads/releases/{file}"),
        )
    }

    /// Create a new release
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `name` - The name of the release
    /// * `description` - The description of the release
    /// * `catalogue_number` - The catalogue number of the release
    /// * `release_date` - The release date of the release
    /// * `record_label_id` - The ID of the record label the release is signed to
    ///
    /// # Returns
    /// The created release
    ///
    /// # Errors
    /// If the release cannot be created, return an error
    /// If the record label is not found, return an error
    #[allow(clippy::too_many_arguments)]
    #[cfg(feature = "ssr")]
    pub async fn create(
        pool: &PgPool,
        name: String,
        description: String,
        primary_artist_id: i64,
        catalogue_number: String,
        release_date: Option<chrono::DateTime<chrono::Utc>>,
        record_label_id: i64,
        published_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> anyhow::Result<Self> {
        let slug = slugify(&name);

        let release = Self {
            id: 0,
            name,
            slug,
            description,
            primary_artist_id,
            primary_image: None,
            catalogue_number,
            release_date,
            label_id: record_label_id,
            published_at,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };
        release.validate(pool).await?;

        let release = sqlx::query_as::<_, Self>(
         "INSERT INTO releases (name, slug, description, primary_artist_id, catalogue_number, release_date, label_id, published_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING *",
     )
         .bind(release.name)
         .bind(release.slug)
         .bind(release.description)
         .bind(release.primary_artist_id)
         .bind(release.catalogue_number)
         .bind(release.release_date)
         .bind(release.label_id)
         .bind(release.published_at)
         .fetch_one(pool)
         .await?;

        Ok(release)
    }

    /// Get release by slug
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `slug` - The slug of the release
    ///
    /// # Returns
    /// The release
    ///
    /// # Errors
    /// If the release cannot be found, return an error
    #[cfg(feature = "ssr")]
    pub async fn get_by_slug(pool: &PgPool, slug: String) -> anyhow::Result<Self> {
        let row = sqlx::query("SELECT * FROM releases WHERE slug = $1")
            .bind(slug.clone())
            .fetch_one(pool)
            .await;

        let row = match row {
            Ok(row) => row,
            Err(e) => {
                tracing::error!("{e}");
                return Err(anyhow::anyhow!(
                    "Could not find release with slug {}.",
                    slug
                ));
            }
        };

        Ok(Self {
            id: row.get("id"),
            name: row.get("name"),
            slug: row.get("slug"),
            description: row.get("description"),
            primary_artist_id: row.get("primary_artist_id"),
            primary_image: row.get("primary_image"),
            catalogue_number: row.get("catalogue_number"),
            release_date: row.get("release_date"),
            label_id: row.get("label_id"),
            published_at: row.get("published_at"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            deleted_at: row.get("deleted_at"),
        })
    }

    /// Get specific release by artist and record label and slug
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `artist_id` - The ID of the artist
    /// * `record_label_id` - The ID of the record label
    /// * `slug` - The slug of the release
    /// * `include_hidden` - Whether to include unreleased releases
    ///
    /// # Returns
    /// The releases
    ///
    /// # Errors
    /// If there is an error getting the releases, return an error
    #[cfg(feature = "ssr")]
    pub async fn get_by_artist_and_record_label_and_slug(
        pool: &PgPool,
        artist_id: i64,
        record_label_id: i64,
        slug: String,
        include_hidden: bool,
    ) -> anyhow::Result<Self> {
        let query = if include_hidden {
            "SELECT releases.* FROM releases
             INNER JOIN release_artists
             ON releases.id = release_artists.release_id
             WHERE release_artists.artist_id = $1 AND releases.label_id = $2 AND releases.slug = $3
             ORDER BY deleted_at DESC, name ASC"
        } else {
            "SELECT releases.* FROM releases
             INNER JOIN release_artists
             ON releases.id = release_artists.release_id
             WHERE release_artists.artist_id = $1 AND releases.label_id = $2 AND releases.slug = $3
              AND deleted_at IS NULL
              AND published_at < NOW()
              AND published_at IS NOT NULL
             ORDER BY name ASC"
        };

        let release = sqlx::query_as::<_, Self>(query)
            .bind(artist_id)
            .bind(record_label_id)
            .bind(slug.clone())
            .fetch_one(pool)
            .await;

        match release {
            Ok(release) => Ok(release),
            Err(e) => {
                tracing::error!("{e}");
                Err(anyhow::anyhow!(
                    "Could not find release {} for artist with id {} and record label with id {}.",
                    slug,
                    artist_id,
                    record_label_id,
                ))
            }
        }
    }

    /// Get next release by (optional) artist and record label
    /// If there are no upcoming releases, return None
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `artist_id` - The ID of the artist (optional)
    /// * `record_label_id` - The ID of the record label (optional)
    ///
    /// # Returns
    /// The next release, if it exists
    ///
    /// # Errors
    /// If there is an error getting the next release, return an error
    #[cfg(feature = "ssr")]
    pub async fn get_next_scheduled_release(
        pool: &PgPool,
        artist_id: Option<i64>,
        record_label_id: i64,
    ) -> anyhow::Result<Option<Self>> {
        let mut query = "SELECT * FROM releases WHERE release_date > NOW() AND published_at IS NOT NULL AND label_id = $1".to_string();
        if artist_id.is_some() {
            query.push_str(" AND primary_artist_id = $2");
        }
        query.push_str(" ORDER BY release_date ASC LIMIT 1");

        let mut query_builder = sqlx::query_as::<_, Self>(&query).bind(record_label_id);
        if let Some(artist_id) = artist_id {
            query_builder = query_builder.bind(artist_id);
        }

        let release = query_builder.fetch_optional(pool).await;
        match release {
            Ok(release) => Ok(release),
            Err(e) => {
                tracing::error!("{e}");
                Err(anyhow::anyhow!("Could not find next scheduled release."))
            }
        }
    }

    /// List releases by artist and record label
    /// This is used to get all releases by an artist on a record label
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `artist_id` - The ID of the artist
    /// * `record_label_id` - The ID of the record label
    /// * `include_hidden` - Whether to include unreleased releases
    ///
    /// # Returns
    /// The releases
    ///
    /// # Errors
    /// If there is an error getting the releases, return an error
    #[cfg(feature = "ssr")]
    pub async fn list_by_artist_and_record_label(
        pool: &PgPool,
        artist_id: i64,
        record_label_id: i64,
        include_hidden: bool,
    ) -> anyhow::Result<Vec<Self>> {
        let query = if include_hidden {
            "SELECT releases.* FROM releases
             INNER JOIN release_artists
             ON releases.id = release_artists.release_id
             WHERE release_artists.artist_id = $1 AND releases.label_id = $2
             ORDER BY deleted_at DESC, release_date DESC, name ASC"
        } else {
            "SELECT releases.* FROM releases
             INNER JOIN release_artists
             ON releases.id = release_artists.release_id
             WHERE release_artists.artist_id = $1 AND releases.label_id = $2
              AND deleted_at IS NULL
              AND published_at < NOW()
              AND published_at IS NOT NULL
             ORDER BY release_date DESC, name ASC"
        };

        let releases = sqlx::query_as::<_, Self>(query)
            .bind(artist_id)
            .bind(record_label_id)
            .fetch_all(pool)
            .await;

        match releases {
            Ok(releases) => Ok(releases),
            Err(e) => {
                tracing::error!("{e}");
                Err(anyhow::anyhow!(
                    "Could not find releases for artist with id {} and record label with id {}.",
                    artist_id,
                    record_label_id
                ))
            }
        }
    }

    /// Update an release
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    ///
    /// # Returns
    /// The updated release
    ///
    /// # Errors
    /// If the release cannot be updated, return an error
    ///
    /// # Panics
    /// If the release cannot be updated, return an error
    #[cfg(feature = "ssr")]
    pub async fn update(mut self, pool: &PgPool) -> anyhow::Result<Self> {
        self.slug = slugify(&self.name);
        self.validate(pool).await?;

        let release = match sqlx::query_as::<_, Self>(
            "UPDATE releases SET name = $1, slug = $2, description = $3, primary_artist_id = $4, primary_image = $5, catalogue_number = $6, release_date = $7, published_at = $8, updated_at = $9, deleted_at = $10 WHERE id = $11 RETURNING *",
        )
        .bind(self.name)
        .bind(self.slug)
        .bind(self.description)
        .bind(self.primary_artist_id)
        .bind(self.primary_image)
        .bind(self.catalogue_number)
        .bind(self.release_date)
        .bind(self.published_at)
        .bind(chrono::Utc::now())
        .bind(self.deleted_at)
        .bind(self.id)
        .fetch_one(pool)
        .await {
            Ok(release) => release,
            Err(e) => {
                tracing::error!("{e}");
                return Err(anyhow::anyhow!(
                    "Could not update release with id {}. {e}",
                    self.id
                ));
            }
        };

        Ok(release)
    }

    /// Delete an release
    /// This is a soft delete
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    ///
    /// # Returns
    /// The deleted release
    ///
    /// # Errors
    /// If the release cannot be deleted, return an error
    #[cfg(feature = "ssr")]
    pub async fn delete(&self, pool: &PgPool) -> anyhow::Result<Self> {
        let release = sqlx::query_as::<_, Self>(
            "UPDATE releases SET deleted_at = $1 WHERE id = $2 RETURNING *",
        )
        .bind(chrono::Utc::now())
        .bind(self.id)
        .fetch_one(pool)
        .await;

        match release {
            Ok(release) => Ok(release),
            Err(e) => {
                tracing::error!("{e}");
                Err(anyhow::anyhow!(
                    "Could not delete release with id {}.",
                    self.id
                ))
            }
        }
    }

    /// Set the artists for the release
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `artist_ids` - The IDs of the artists
    /// # Returns
    /// The release
    /// # Errors
    /// If the release cannot be updated, return an error
    /// # Panics
    /// If the release cannot be updated, return an error
    #[cfg(feature = "ssr")]
    pub async fn set_artists(&self, pool: &PgPool, artist_ids: Vec<i64>) -> anyhow::Result<Self> {
        if artist_ids.is_empty() {
            return Err(anyhow::anyhow!("Artist IDs cannot be empty."));
        }

        let mut tx = pool.begin().await?;

        // Delete all artists for the release
        sqlx::query("DELETE FROM release_artists WHERE release_id = $1")
            .bind(self.id)
            .execute(&mut *tx)
            .await?;

        // Insert the new artists
        for artist_id in artist_ids {
            match sqlx::query("INSERT INTO release_artists (release_id, artist_id) VALUES ($1, $2)")
                .bind(self.id)
                .bind(artist_id)
                .execute(&mut *tx)
                .await
            {
                Ok(_) => (),
                Err(e) => {
                    tracing::error!("{e}");
                    return Err(anyhow::anyhow!(
                        "Could not set artists for release with id {}.",
                        self.id
                    ));
                }
            }
        }

        let _ = tx.commit().await;

        Ok(self.clone())
    }

    /// Get the artists for the release
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    ///
    /// # Returns
    /// The artists for the release
    ///
    /// # Errors
    /// If the release cannot be found, return an error
    /// # Panics
    /// If the release cannot be found, return an error
    #[cfg(feature = "ssr")]
    pub async fn get_artists(&self, pool: &PgPool) -> anyhow::Result<Vec<Artist>> {
        let artists = sqlx::query_as::<_, Artist>(
            "SELECT artists.* FROM artists
             INNER JOIN release_artists ON artists.id = release_artists.artist_id
             WHERE release_artists.release_id = $1",
        )
        .bind(self.id)
        .fetch_all(pool)
        .await?;

        Ok(artists)
    }

    /// Set the tracks for the release
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `track_ids` - The IDs of the tracks
    /// # Returns
    /// The release
    /// # Errors
    /// If the release cannot be updated, return an error
    /// # Panics
    /// If the release cannot be updated, return an error
    #[cfg(feature = "ssr")]
    pub async fn set_tracks(&self, pool: &PgPool, track_ids: Vec<i64>) -> anyhow::Result<Self> {
        let mut tx = pool.begin().await?;

        // Delete all tracks for the release
        sqlx::query("DELETE FROM release_tracks WHERE release_id = $1")
            .bind(self.id)
            .execute(&mut *tx)
            .await?;

        // Insert the new tracks
        for track_id in track_ids {
            match sqlx::query("INSERT INTO release_tracks (release_id, track_id) VALUES ($1, $2)")
                .bind(self.id)
                .bind(track_id)
                .execute(&mut *tx)
                .await
            {
                Ok(_) => (),
                Err(e) => {
                    tracing::error!("{e}");
                    return Err(anyhow::anyhow!(
                        "Could not set tracks for release with id {}.",
                        self.id
                    ));
                }
            }
        }

        let _ = tx.commit().await;

        Ok(self.clone())
    }

    /// Get the tracks for the release
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    ///
    /// # Returns
    /// The tracks for the release
    ///
    /// # Errors
    /// If the release cannot be found, return an error
    /// # Panics
    /// If the release cannot be found, return an error
    #[cfg(feature = "ssr")]
    pub async fn get_tracks(&self, pool: &PgPool) -> anyhow::Result<Vec<TrackWithArtists>> {
        let tracks = sqlx::query_as::<_, Track>(
            "SELECT tracks.* FROM tracks
             INNER JOIN release_tracks ON tracks.id = release_tracks.track_id
             WHERE release_tracks.release_id = $1",
        )
        .bind(self.id)
        .fetch_all(pool)
        .await?;

        tracing::debug!("Loaded {} tracks for release {}", tracks.len(), self.name);
        let mut tracks_with_artists: Vec<TrackWithArtists> = Vec::new();
        for track in &tracks {
            // Ensure each track has its artists loaded
            let artists = track.get_artists(pool).await?;

            tracks_with_artists.push(TrackWithArtists {
                track: track.clone(),
                artists: artists.clone(),
            });
            tracing::debug!("Loaded {} artists for track {}", artists.len(), track.name);
        }

        Ok(tracks_with_artists)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let release = Release {
            id: 1,
            name: "Test Release".to_string(),
            slug: "test-release".to_string(),
            description: "This is a test release".to_string(),
            primary_artist_id: artist.id,
            primary_image: None,
            catalogue_number: "TEST-0001".to_string(),
            release_date: Some(chrono::Utc::now()),
            label_id: record_label.id,
            published_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let result = release.validate(&pool).await;

        assert!(result.is_ok());
    }

    #[sqlx::test]
    async fn test_validate_name_is_empty(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let release = Release {
            id: 1,
            name: String::new(),
            slug: "test-release".to_string(),
            description: "This is a test release".to_string(),
            primary_artist_id: artist.id,
            primary_image: None,
            catalogue_number: "TEST-0001".to_string(),
            release_date: Some(chrono::Utc::now()),
            label_id: 1,
            published_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let result = release.validate(&pool).await;

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
        let release = Release {
            id: 1,
            name,
            slug: "test-release".to_string(),
            description: "This is a test release".to_string(),
            primary_artist_id: artist.id,
            primary_image: None,
            catalogue_number: "TEST-0001".to_string(),
            release_date: Some(chrono::Utc::now()),
            label_id: 1,
            published_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let result = release.validate(&pool).await;

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
        let release = Release {
            id: 1,
            name: "Test Release".to_string(),
            slug,
            description: "This is a test release".to_string(),
            primary_artist_id: artist.id,
            primary_image: None,
            catalogue_number: "TEST-0001".to_string(),
            release_date: Some(chrono::Utc::now()),
            label_id: 1,
            published_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let result = release.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Slug must be less than 255 characters.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_validate_slug_unique(pool: PgPool) {
        let release = create_test_release(&pool, 1, None).await.unwrap();
        let mut new_release = release.clone();
        new_release.id = 2;
        new_release.slug = release.slug.clone();

        let result = new_release.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Slug must be unique.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_primary_artist_does_not_exist(pool: PgPool) {
        let release = Release {
            id: 1,
            name: "Test Release".to_string(),
            slug: "test-release".to_string(),
            description: "This is a test release".to_string(),
            primary_artist_id: 1,
            primary_image: None,
            catalogue_number: "TEST-0001".to_string(),
            release_date: Some(chrono::Utc::now()),
            label_id: 1,
            published_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let result = release.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Artist with id 1 does not exist.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_validate_catalogue_number_length(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();

        let catalogue_number = "a".repeat(256);
        let release = Release {
            id: 1,
            name: "Test Release".to_string(),
            slug: "test-release".to_string(),
            description: "This is a test release".to_string(),
            primary_artist_id: artist.id,
            primary_image: None,
            catalogue_number,
            release_date: Some(chrono::Utc::now()),
            label_id: 1,
            published_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let result = release.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Catalogue number must be less than 255 characters.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_validate_catalogue_number_unique(pool: PgPool) {
        let release = create_test_release(&pool, 1, None).await.unwrap();
        let mut new_release = release.clone();
        new_release.id = 2;
        new_release.slug = "new-release-2".to_string();
        new_release.catalogue_number = release.catalogue_number.clone();

        let result = new_release.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Catalogue number must be unique.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_validate_record_label_exists(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();

        let release = Release {
            id: 1,
            name: "Test Release".to_string(),
            slug: "test-release".to_string(),
            description: "This is a test release".to_string(),
            primary_artist_id: artist.id,
            primary_image: None,
            catalogue_number: "TEST-0001".to_string(),
            release_date: Some(chrono::Utc::now()),
            label_id: 100,
            published_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let result = release.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Record Label with id 100 does not exist.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_create(pool: PgPool) {
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();
        let release = Release::create(
            &pool,
            "Test Release".to_string(),
            "This is a test release".to_string(),
            artist.id,
            "TEST-0001".to_string(),
            None,
            record_label.id,
            Some(chrono::Utc::now()),
        )
        .await
        .unwrap();

        assert_eq!(release.name, "Test Release".to_string());
        assert_eq!(release.description, "This is a test release".to_string());
    }

    #[sqlx::test]
    async fn test_create_with_validation_error(pool: PgPool) {
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();
        let release = Release::create(
            &pool,
            String::new(),
            "This is a test release".to_string(),
            artist.id,
            "TEST-0001".to_string(),
            Some(chrono::Utc::now()),
            record_label.id,
            Some(chrono::Utc::now()),
        )
        .await;

        assert!(release.is_err());
        assert_eq!(
            release.unwrap_err().to_string(),
            "Name is required.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_get_by_slug(pool: PgPool) {
        let release = create_test_release(&pool, 1, None).await.unwrap();
        let release_by_slug = Release::get_by_slug(&pool, release.slug.clone())
            .await
            .unwrap();

        assert_eq!(release, release_by_slug);
    }

    #[sqlx::test]
    async fn test_get_by_slug_not_found(pool: PgPool) {
        let release = Release::get_by_slug(&pool, "missing".to_string()).await;

        assert!(release.is_err());
        assert_eq!(
            release.unwrap_err().to_string(),
            "Could not find release with slug missing.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_get_by_artist_and_record_label_and_slug_no_releases(pool: PgPool) {
        let release =
            Release::get_by_artist_and_record_label_and_slug(&pool, 1, 1, "test".to_string(), true)
                .await;

        assert!(release.is_err());
        assert_eq!(
            release.unwrap_err().to_string(),
            "Could not find release test for artist with id 1 and record label with id 1."
                .to_string()
        );
    }

    #[sqlx::test]
    async fn test_get_by_artist_and_record_label_and_slug(pool: PgPool) {
        let test_release = create_test_release(&pool, 1, None).await.unwrap();
        let release = Release::get_by_artist_and_record_label_and_slug(
            &pool,
            1,
            1,
            test_release.slug.clone(),
            true,
        )
        .await
        .unwrap();

        assert_eq!(release.id, test_release.id);
        assert_eq!(release.slug, test_release.slug);
    }

    #[sqlx::test]
    async fn test_get_by_artist_and_record_label_and_slug_deleted_release_include_hidden(
        pool: PgPool,
    ) {
        let deleted_release = create_test_release(&pool, 1, None).await.unwrap();
        deleted_release.delete(&pool).await.unwrap();
        let release = Release::get_by_artist_and_record_label_and_slug(
            &pool,
            1,
            1,
            deleted_release.slug.clone(),
            true,
        )
        .await
        .unwrap();

        assert_eq!(release.id, deleted_release.id);
    }

    #[sqlx::test]
    async fn test_get_by_artist_and_record_label_and_slug_deleted_release(pool: PgPool) {
        let deleted_release = create_test_release(&pool, 1, None).await.unwrap();
        deleted_release.delete(&pool).await.unwrap();
        let release = Release::get_by_artist_and_record_label_and_slug(
            &pool,
            1,
            1,
            deleted_release.slug.clone(),
            false,
        )
        .await;

        assert!(release.is_err());
        assert_eq!(
            release.unwrap_err().to_string(),
            "Could not find release test-release-1 for artist with id 1 and record label with id 1.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_get_by_artist_and_record_label_and_slug_unpublished_release_include_hidden(
        pool: PgPool,
    ) {
        let mut unpublished_release = create_test_release(&pool, 1, None).await.unwrap();
        unpublished_release.published_at = None;
        unpublished_release.clone().update(&pool).await.unwrap();
        let release = Release::get_by_artist_and_record_label_and_slug(
            &pool,
            1,
            1,
            unpublished_release.slug.clone(),
            true,
        )
        .await
        .unwrap();

        assert_eq!(release.id, unpublished_release.id);
        assert_eq!(release.slug, unpublished_release.slug);
    }

    #[sqlx::test]
    async fn test_get_by_artist_and_record_label_and_slug_unpublished_release(pool: PgPool) {
        let mut unpublished_release = create_test_release(&pool, 1, None).await.unwrap();
        unpublished_release.published_at = None;
        unpublished_release.clone().update(&pool).await.unwrap();
        let release = Release::get_by_artist_and_record_label_and_slug(
            &pool,
            1,
            1,
            unpublished_release.slug.clone(),
            false,
        )
        .await;

        assert!(release.is_err());
        assert_eq!(
            release.unwrap_err().to_string(),
            "Could not find release test-release-1 for artist with id 1 and record label with id 1.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_get_by_artist_and_record_label_and_slug_future_release_include_hidden(
        pool: PgPool,
    ) {
        let mut future_release = create_test_release(&pool, 1, None).await.unwrap();
        future_release.published_at = Some(chrono::Utc::now() + chrono::Duration::days(1));
        future_release.clone().update(&pool).await.unwrap();
        let release = Release::get_by_artist_and_record_label_and_slug(
            &pool,
            1,
            1,
            future_release.slug.clone(),
            true,
        )
        .await
        .unwrap();

        assert_eq!(release.id, future_release.id);
        assert_eq!(release.slug, future_release.slug);
    }

    #[sqlx::test]
    async fn test_get_by_artist_and_record_label_and_slug_future_release(pool: PgPool) {
        let mut future_release = create_test_release(&pool, 1, None).await.unwrap();
        future_release.published_at = Some(chrono::Utc::now() + chrono::Duration::days(1));
        future_release.clone().update(&pool).await.unwrap();
        let release = Release::get_by_artist_and_record_label_and_slug(
            &pool,
            1,
            1,
            future_release.slug.clone(),
            false,
        )
        .await;

        assert!(release.is_err());
        assert_eq!(
            release.unwrap_err().to_string(),
            "Could not find release test-release-1 for artist with id 1 and record label with id 1.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_get_next_scheduled_release_no_releases(pool: PgPool) {
        let release = Release::get_next_scheduled_release(&pool, None, 1)
            .await
            .unwrap();
        assert!(release.is_none());
    }

    #[sqlx::test]
    async fn test_get_next_scheduled_release_with_releases(pool: PgPool) {
        let mut release = create_test_release(&pool, 1, None).await.unwrap();
        release.release_date = Some(chrono::Utc::now() + chrono::Duration::days(1));
        release.clone().update(&pool).await.unwrap();

        let next_release = Release::get_next_scheduled_release(&pool, None, release.label_id)
            .await
            .unwrap();

        assert!(next_release.is_some());
        assert_eq!(next_release.unwrap().id, release.id);
    }

    #[sqlx::test]
    async fn test_get_next_scheduled_release_with_artist(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let mut release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();
        release.release_date = Some(chrono::Utc::now() + chrono::Duration::days(2));
        release.clone().update(&pool).await.unwrap();
        let other_artist = create_test_artist(&pool, 2, None).await.unwrap();
        let mut other_upcoming_release = create_test_release(&pool, 2, Some(other_artist.clone()))
            .await
            .unwrap();
        other_upcoming_release.release_date = Some(chrono::Utc::now() + chrono::Duration::days(1));
        other_upcoming_release.clone().update(&pool).await.unwrap();

        let next_release =
            Release::get_next_scheduled_release(&pool, Some(artist.id), release.label_id)
                .await
                .unwrap();

        assert!(next_release.is_some());
        assert_eq!(next_release.unwrap().id, release.id);
    }

    #[sqlx::test]
    async fn test_list_by_artist_and_record_label_no_releases(pool: PgPool) {
        let releases = Release::list_by_artist_and_record_label(&pool, 1, 1, true)
            .await
            .unwrap();

        assert_eq!(releases.len(), 0);
    }

    #[sqlx::test]
    async fn test_list_by_artist_and_record_label_with_releases(pool: PgPool) {
        let release = create_test_release(&pool, 1, None).await.unwrap();
        let releases = Release::list_by_artist_and_record_label(&pool, 1, 1, true)
            .await
            .unwrap();

        assert_eq!(releases.len(), 1);
        assert_eq!(releases[0].id, release.id);
    }

    #[sqlx::test]
    async fn test_list_by_artist_and_record_label_with_unpublished_releases(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let record_label_id = artist.label_id;
        let mut unpublished_release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();
        unpublished_release.published_at = None;
        unpublished_release.update(&pool).await.unwrap();
        let mut published_in_future_release = create_test_release(&pool, 2, Some(artist.clone()))
            .await
            .unwrap();
        published_in_future_release.published_at =
            Some(chrono::Utc::now() + chrono::Duration::days(1));
        published_in_future_release.update(&pool).await.unwrap();
        let mut published_release = create_test_release(&pool, 3, Some(artist.clone()))
            .await
            .unwrap();
        published_release.published_at = Some(chrono::Utc::now() - chrono::Duration::days(1));
        published_release.clone().update(&pool).await.unwrap();
        let deleted_release = create_test_release(&pool, 4, Some(artist.clone()))
            .await
            .unwrap();
        deleted_release.delete(&pool).await.unwrap();

        let releases =
            Release::list_by_artist_and_record_label(&pool, artist.id, record_label_id, false)
                .await
                .unwrap();

        assert_eq!(releases.len(), 1);
        assert_eq!(releases[0].id, published_release.id);
    }

    #[sqlx::test]
    async fn test_list_by_artist_and_record_label_with_unpublished_releases_show_all(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let record_label_id = artist.label_id;
        let mut unpublished_release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();
        unpublished_release.published_at = None;
        unpublished_release.release_date = None;
        unpublished_release.clone().update(&pool).await.unwrap();
        let mut published_in_future_release = create_test_release(&pool, 2, Some(artist.clone()))
            .await
            .unwrap();
        published_in_future_release.published_at =
            Some(chrono::Utc::now() + chrono::Duration::days(1));
        published_in_future_release.release_date =
            Some(chrono::Utc::now() + chrono::Duration::days(1));
        published_in_future_release
            .clone()
            .update(&pool)
            .await
            .unwrap();
        let mut published_release = create_test_release(&pool, 3, Some(artist.clone()))
            .await
            .unwrap();
        published_release.published_at = Some(chrono::Utc::now() - chrono::Duration::days(1));
        published_release.release_date = Some(chrono::Utc::now() - chrono::Duration::days(1));
        published_release.clone().update(&pool).await.unwrap();
        let deleted_release = create_test_release(&pool, 4, Some(artist.clone()))
            .await
            .unwrap();
        deleted_release.clone().delete(&pool).await.unwrap();

        let releases =
            Release::list_by_artist_and_record_label(&pool, artist.id, record_label_id, true)
                .await
                .unwrap();

        assert_eq!(releases.len(), 4);
        assert_eq!(releases[0].id, unpublished_release.id);
        assert_eq!(releases[1].id, published_in_future_release.id);
        assert_eq!(releases[2].id, published_release.id);
        assert_eq!(releases[3].id, deleted_release.id);
    }

    #[sqlx::test]
    async fn test_list_by_artist_and_record_label_wrong_artist(pool: PgPool) {
        create_test_release(&pool, 1, None).await.unwrap();
        let releases = Release::list_by_artist_and_record_label(&pool, 2, 1, true)
            .await
            .unwrap();

        assert_eq!(releases.len(), 0);
    }

    #[sqlx::test]
    async fn test_list_by_artist_and_record_label_wrong_label(pool: PgPool) {
        create_test_release(&pool, 1, None).await.unwrap();
        let releases = Release::list_by_artist_and_record_label(&pool, 1, 2, true)
            .await
            .unwrap();

        assert_eq!(releases.len(), 0);
    }

    #[sqlx::test]
    async fn test_update(pool: PgPool) {
        let release = create_test_release(&pool, 1, None).await.unwrap();
        let mut update_release = release.clone();
        update_release.name = "Updated Release".to_string();
        update_release.description = "This is an updated release".to_string();
        update_release.primary_image = Some("an-image.jpg".to_string());
        update_release.catalogue_number = "UPDATED-0001".to_string();
        update_release.release_date = Some(chrono::Utc::now());

        let updated_release = update_release.update(&pool).await.unwrap();
        assert_eq!(updated_release.name, "Updated Release".to_string());
        assert_eq!(updated_release.slug, "updated-release".to_string());
        assert_eq!(
            updated_release.description,
            "This is an updated release".to_string()
        );
        assert_eq!(
            updated_release.primary_image,
            Some("an-image.jpg".to_string())
        );
        assert_ne!(updated_release.updated_at, release.updated_at);
    }

    #[sqlx::test]
    async fn test_update_validation_error(pool: PgPool) {
        let release = create_test_release(&pool, 1, None).await.unwrap();
        let mut update_release = release.clone();
        update_release.name = String::new();
        let updated_release = update_release.update(&pool).await;

        assert!(updated_release.is_err());
        assert_eq!(
            updated_release.unwrap_err().to_string(),
            "Name is required.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_delete(pool: PgPool) {
        let release = create_test_release(&pool, 1, None).await.unwrap();
        let result = release.delete(&pool).await.unwrap();
        assert!(result.deleted_at.is_some());
    }

    #[sqlx::test]
    async fn test_delete_not_found(pool: PgPool) {
        let release = Release::default();
        let result = release.delete(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Could not delete release with id 0.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_set_artists(pool: PgPool) {
        let release = create_test_release(&pool, 1, None).await.unwrap();
        let artist = release.get_artists(&pool).await.unwrap()[0].clone();
        let record_label = RecordLabel::get_by_id(&pool, artist.label_id)
            .await
            .unwrap();
        let artist2 = create_test_artist(&pool, 2, Some(record_label))
            .await
            .unwrap();

        let result = release
            .set_artists(&pool, vec![artist.id, artist2.id])
            .await;

        assert!(result.is_ok());
    }

    #[sqlx::test]
    async fn test_set_artists_not_found(pool: PgPool) {
        let release = Release::default();
        let result = release.set_artists(&pool, vec![1, 2]).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Could not set artists for release with id 0.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_set_artists_no_artists(pool: PgPool) {
        let release = create_test_release(&pool, 1, None).await.unwrap();
        let result = release.set_artists(&pool, vec![]).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Artist IDs cannot be empty.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_set_artists_replace(pool: PgPool) {
        let release = create_test_release(&pool, 1, None).await.unwrap();
        let artist = release.get_artists(&pool).await.unwrap()[0].clone();
        let record_label = RecordLabel::get_by_id(&pool, artist.label_id)
            .await
            .unwrap();
        let artist2 = create_test_artist(&pool, 2, Some(record_label.clone()))
            .await
            .unwrap();
        let artist3 = create_test_artist(&pool, 3, Some(record_label))
            .await
            .unwrap();

        // Set the artists for the release
        release
            .set_artists(&pool, vec![artist.id, artist2.id])
            .await
            .unwrap();

        // Replace the artists for the release
        let result = release.set_artists(&pool, vec![artist3.id]).await;

        assert!(result.is_ok());
        let artists = release.get_artists(&pool).await.unwrap();
        assert_eq!(artists.len(), 1);
        assert_eq!(artists[0].id, artist3.id);
    }

    /// Test `get_artists`
    #[sqlx::test]
    async fn test_get_artists(pool: PgPool) {
        let release = create_test_release(&pool, 1, None).await.unwrap();
        let artist = release.get_artists(&pool).await.unwrap()[0].clone();
        let record_label = RecordLabel::get_by_id(&pool, artist.label_id)
            .await
            .unwrap();
        let artist2 = create_test_artist(&pool, 2, Some(record_label))
            .await
            .unwrap();

        // Set the artists for the release
        release
            .set_artists(&pool, vec![artist.id, artist2.id])
            .await
            .unwrap();

        // Get the artists for the release
        let artists = release.get_artists(&pool).await.unwrap();

        assert_eq!(artists.len(), 2);
        assert_eq!(artists[0].id, artist.id);
        assert_eq!(artists[1].id, artist2.id);
    }

    #[sqlx::test]
    async fn test_set_tracks(pool: PgPool) {
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label))
            .await
            .unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();
        let track = create_test_track(&pool, 1, Some(release.clone()), Some(artist.clone()))
            .await
            .unwrap();
        let track2 = create_test_track(&pool, 2, Some(release.clone()), Some(artist))
            .await
            .unwrap();

        let result = release.set_tracks(&pool, vec![track.id, track2.id]).await;

        assert!(result.is_ok());
    }

    #[sqlx::test]
    async fn test_set_tracks_not_found(pool: PgPool) {
        let release = Release::default();
        let result = release.set_tracks(&pool, vec![1, 2]).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Could not set tracks for release with id 0.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_set_tracks_no_tracks(pool: PgPool) {
        let release = create_test_release(&pool, 1, None).await.unwrap();
        let result = release.set_tracks(&pool, vec![]).await;

        assert!(result.is_ok());
        assert!(result.unwrap().get_tracks(&pool).await.unwrap().is_empty());
    }

    #[sqlx::test]
    async fn test_set_tracks_replace(pool: PgPool) {
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label))
            .await
            .unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();
        let track = create_test_track(&pool, 1, Some(release.clone()), Some(artist.clone()))
            .await
            .unwrap();
        let track2 = create_test_track(&pool, 2, Some(release.clone()), Some(artist.clone()))
            .await
            .unwrap();
        let track3 = create_test_track(&pool, 3, Some(release.clone()), Some(artist.clone()))
            .await
            .unwrap();

        // Set the tracks for the release
        release
            .set_tracks(&pool, vec![track.id, track2.id])
            .await
            .unwrap();

        // Replace the tracks for the release
        let result = release.set_tracks(&pool, vec![track3.id]).await;

        assert!(result.is_ok());
        let tracks = release.get_tracks(&pool).await.unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].track.id, track3.id);
        assert_eq!(tracks[0].artists.len(), 1);
        assert_eq!(tracks[0].artists[0].id, artist.id);
    }

    /// Test `get_tracks`
    #[sqlx::test]
    async fn test_get_tracks(pool: PgPool) {
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label))
            .await
            .unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();
        let track = create_test_track(&pool, 1, Some(release.clone()), Some(artist.clone()))
            .await
            .unwrap();
        let track2 = create_test_track(&pool, 2, Some(release.clone()), Some(artist.clone()))
            .await
            .unwrap();

        // Set the tracks for the release
        release
            .set_tracks(&pool, vec![track.id, track2.id])
            .await
            .unwrap();

        // Get the tracks for the release
        let tracks = release.get_tracks(&pool).await.unwrap();

        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].track.id, track.id);
        assert_eq!(tracks[1].track.id, track2.id);
        assert_eq!(tracks[0].artists.len(), 1);
        assert_eq!(tracks[1].artists.len(), 1);
        assert_eq!(tracks[0].artists[0].id, artist.clone().id);
        assert_eq!(tracks[1].artists[0].id, artist.id);
    }

    #[sqlx::test]
    async fn test_primary_image_url(pool: PgPool) {
        let release = create_test_release(&pool, 1, None).await.unwrap();
        let url = release.primary_image_url();
        assert_eq!(url, "/Logo.svg");
    }

    #[sqlx::test]
    async fn test_primary_image_url_with_custom_image(pool: PgPool) {
        let release = create_test_release(&pool, 1, None).await.unwrap();
        let mut release = release;
        release.primary_image = Some("custom-image.jpg".to_string());
        let url = release.primary_image_url();
        assert_eq!(url, "/uploads/releases/custom-image.jpg");
    }
}
