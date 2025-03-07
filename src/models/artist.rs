//! Artist model
//!
//! The Artist struct is used to represent a record artist in the database.

use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use sqlx::{FromRow, PgPool, Row};

#[cfg(feature = "ssr")]
use super::record_label::RecordLabel;
use super::traits::Validate;
#[cfg(feature = "ssr")]
use crate::utils::slugify::slugify;

/// The Artist struct is used to represent a record artist in the database.
#[derive(Serialize, Deserialize, Clone, Default, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "ssr", derive(FromRow))]
pub struct Artist {
    /// The unique identifier of the artist
    pub id: i64,
    /// The name of the artist
    pub name: String,
    /// The slug of the artist
    pub slug: String,
    /// The description of the artist
    pub description: String,
    /// The label id
    pub label_id: i64,
    /// The date the artist is published.
    /// If this is None, the artist is not published
    /// If this is in the future, the artist is scheduled to be published
    /// If this is in the past, the artist is published
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
    /// The date and time the artist was created in the database
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// The date and time the artist was last updated
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// The date and time the artist was deleted
    /// If this is None, the artist is not deleted
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl Validate for Artist {
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
        if let Ok(artist) = Self::get_by_slug(pool, self.slug.clone()).await {
            if artist.id != self.id {
                return Err(anyhow::anyhow!("Slug must be unique.".to_string()));
            }
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

impl Artist {
    /// Create a new artist
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `name` - The name of the artist
    /// * `description` - The description of the artist
    /// * `record_label_id` - The ID of the record label the artist is signed to
    ///
    /// # Returns
    /// The created artist
    ///
    /// # Errors
    /// If the artist cannot be created, return an error
    /// If the record label is not found, return an error
    #[cfg(feature = "ssr")]
    pub async fn create(
        pool: &PgPool,
        name: String,
        description: String,
        record_label_id: i64,
        published_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> anyhow::Result<Self> {
        let slug = slugify(&name);

        let artist = Self {
            id: 0,
            name,
            slug,
            description,
            label_id: record_label_id,
            published_at,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };
        artist.validate(pool).await?;

        let artist = sqlx::query_as::<_, Self>(
         "INSERT INTO artists (name, slug, description, label_id, published_at) VALUES ($1, $2, $3, $4, $5) RETURNING *",
     )
         .bind(artist.name)
         .bind(artist.slug)
         .bind(artist.description)
         .bind(artist.label_id)
         .bind(artist.published_at)
         .fetch_one(pool)
         .await?;

        Ok(artist)
    }

    /// Get artist by slug
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `slug` - The slug of the artist
    ///
    /// # Returns
    /// The artist
    ///
    /// # Errors
    /// If the artist cannot be found, return an error
    #[cfg(feature = "ssr")]
    pub async fn get_by_slug(pool: &PgPool, slug: String) -> anyhow::Result<Self> {
        let row = sqlx::query("SELECT * FROM artists WHERE slug = $1")
            .bind(slug.clone())
            .fetch_one(pool)
            .await;

        let row = match row {
            Ok(row) => row,
            Err(e) => {
                tracing::error!("{e}");
                return Err(anyhow::anyhow!("Could not find artist with slug {}.", slug));
            }
        };

        Ok(Self {
            id: row.get("id"),
            name: row.get("name"),
            slug: row.get("slug"),
            description: row.get("description"),
            label_id: row.get("label_id"),
            published_at: row.get("published_at"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            deleted_at: row.get("deleted_at"),
        })
    }

    /// Update an artist
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    ///
    /// # Returns
    /// The updated artist
    ///
    /// # Errors
    /// If the artist cannot be updated, return an error
    ///
    /// # Panics
    /// If the artist cannot be updated, return an error
    #[cfg(feature = "ssr")]
    pub async fn update(mut self, pool: &PgPool) -> anyhow::Result<Self> {
        self.slug = slugify(&self.name);
        self.validate(pool).await?;

        let artist = match sqlx::query_as::<_, Self>(
            "UPDATE artists SET name = $1, slug = $2, description = $3, published_at = $4, updated_at = $5 WHERE id = $6 RETURNING *",
        )
        .bind(self.name)
        .bind(self.slug)
        .bind(self.description)
        .bind(self.published_at)
        .bind(chrono::Utc::now())
        .bind(self.id)
        .fetch_one(pool)
        .await {
            Ok(artist) => artist,
            Err(e) => {
                tracing::error!("{e}");
                return Err(anyhow::anyhow!(
                    "Could not update artist with id {}.",
                    self.id
                ));
            }
        };

        Ok(artist)
    }

    /// Delete an artist
    /// This is a soft delete
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    ///
    /// # Returns
    /// The deleted artist
    ///
    /// # Errors
    /// If the artist cannot be deleted, return an error
    #[cfg(feature = "ssr")]
    pub async fn delete(&self, pool: &PgPool) -> anyhow::Result<Self> {
        let artist = sqlx::query_as::<_, Self>(
            "UPDATE artists SET deleted_at = $1 WHERE id = $2 RETURNING *",
        )
        .bind(chrono::Utc::now())
        .bind(self.id)
        .fetch_one(pool)
        .await;

        match artist {
            Ok(artist) => Ok(artist),
            Err(e) => {
                eprintln!("{e}");
                Err(anyhow::anyhow!(
                    "Could not delete artist with id {}.",
                    self.id
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "ssr")]
    use crate::models::test_helpers::{create_test_artist, create_test_record_label};

    #[test]
    fn test_init_artist() {
        let artist = Artist {
            id: 1,
            name: "Test Artist".to_string(),
            slug: "test-artist".to_string(),
            description: "This is a test artist".to_string(),
            label_id: 1,
            published_at: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        assert_eq!(artist.id, 1);
        assert_eq!(artist.name, "Test Artist".to_string());
        assert_eq!(artist.slug, "test-artist".to_string());
        assert_eq!(artist.description, "This is a test artist".to_string());
        assert_eq!(artist.label_id, 1);
    }

    #[sqlx::test]
    async fn test_validate_success(pool: PgPool) {
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = Artist {
            id: 1,
            name: "Test Artist".to_string(),
            slug: "test-artist".to_string(),
            description: "This is a test artist".to_string(),
            label_id: record_label.id,
            published_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let result = artist.validate(&pool).await;

        assert!(result.is_ok());
    }

    #[sqlx::test]
    async fn test_validate_name_is_empty(pool: PgPool) {
        let artist = Artist {
            id: 1,
            name: String::new(),
            slug: "test-artist".to_string(),
            description: "This is a test artist".to_string(),
            label_id: 1,
            published_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let result = artist.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Name is required.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_validate_name_length(pool: PgPool) {
        let name = "a".repeat(256);
        let artist = Artist {
            id: 1,
            name,
            slug: "test-artist".to_string(),
            description: "This is a test artist".to_string(),
            label_id: 1,
            published_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let result = artist.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Name must be less than 255 characters.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_validate_slug_length(pool: PgPool) {
        let slug = "a".repeat(256);
        let artist = Artist {
            id: 1,
            name: "Test Artist".to_string(),
            slug,
            description: "This is a test artist".to_string(),
            label_id: 1,
            published_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let result = artist.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Slug must be less than 255 characters.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_validate_slug_unique(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let mut new_artist = artist.clone();
        new_artist.id = 2;
        new_artist.slug = artist.slug.clone();

        let result = new_artist.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Slug must be unique.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_validate_record_label_exists(pool: PgPool) {
        let artist = Artist {
            id: 1,
            name: "Test Artist".to_string(),
            slug: "test-artist".to_string(),
            description: "This is a test artist".to_string(),
            label_id: 1,
            published_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let result = artist.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Record Label with id 1 does not exist.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_create(pool: PgPool) {
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = Artist::create(
            &pool,
            "Test Artist".to_string(),
            "This is a test artist".to_string(),
            record_label.id,
            Some(chrono::Utc::now()),
        )
        .await
        .unwrap();

        assert_eq!(artist.name, "Test Artist".to_string());
        assert_eq!(artist.description, "This is a test artist".to_string());
    }

    #[sqlx::test]
    async fn test_create_with_validation_error(pool: PgPool) {
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = Artist::create(
            &pool,
            String::new(),
            "This is a test artist".to_string(),
            record_label.id,
            Some(chrono::Utc::now()),
        )
        .await;

        assert!(artist.is_err());
        assert_eq!(
            artist.unwrap_err().to_string(),
            "Name is required.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_get_by_slug(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let artist_by_slug = Artist::get_by_slug(&pool, artist.slug.clone())
            .await
            .unwrap();

        assert_eq!(artist, artist_by_slug);
    }

    #[sqlx::test]
    async fn test_get_by_slug_not_found(pool: PgPool) {
        let artist = Artist::get_by_slug(&pool, "missing".to_string()).await;

        assert!(artist.is_err());
        assert_eq!(
            artist.unwrap_err().to_string(),
            "Could not find artist with slug missing.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_update(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let mut update_artist = artist.clone();
        update_artist.name = "Updated Artist".to_string();
        update_artist.description = "This is an updated artist".to_string();

        let updated_artist = update_artist.update(&pool).await.unwrap();
        assert_eq!(updated_artist.name, "Updated Artist".to_string());
        assert_eq!(updated_artist.slug, "updated-artist".to_string());
        assert_eq!(
            updated_artist.description,
            "This is an updated artist".to_string()
        );
        assert_ne!(updated_artist.updated_at, artist.updated_at);
    }

    #[sqlx::test]
    async fn test_update_validation_error(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let mut update_artist = artist.clone();
        update_artist.name = String::new();
        let updated_artist = update_artist.update(&pool).await;

        assert!(updated_artist.is_err());
        assert_eq!(
            updated_artist.unwrap_err().to_string(),
            "Name is required.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_delete(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let result = artist.delete(&pool).await.unwrap();
        assert!(result.deleted_at.is_some());
    }

    #[sqlx::test]
    async fn test_delete_not_found(pool: PgPool) {
        let artist = Artist::default();
        let result = artist.delete(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Could not delete artist with id 0.".to_string()
        );
    }
}
