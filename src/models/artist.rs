//! Artist model
//!
//! The Artist struct is used to represent a record artist in the database.

use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use sqlx::{FromRow, PgPool, Row};

#[cfg(feature = "ssr")]
use super::record_label::RecordLabel;
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
    /// The date and time the artist was created in the database
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// The date and time the artist was last updated
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// The date and time the artist was deleted
    /// If this is None, the artist is not deleted
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
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
    ) -> anyhow::Result<Self> {
        let slug = slugify(&name);

        let record_label = match RecordLabel::get_by_id(pool, record_label_id).await {
            Ok(record_label) => record_label,
            Err(e) => {
                eprintln!("{e}");
                return Err(anyhow::anyhow!(
                    "Could not find record label with id {}",
                    record_label_id
                ));
            }
        };

        let artist = sqlx::query_as::<_, Self>(
         "INSERT INTO artists (name, slug, description, label_id) VALUES ($1, $2, $3, $4) RETURNING *",
     )
         .bind(name)
         .bind(slug)
         .bind(description)
         .bind(record_label.id)
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
                eprintln!("{e}");
                return Err(anyhow::anyhow!("Could not find artist with slug {}", slug));
            }
        };

        Ok(Self {
            id: row.get("id"),
            name: row.get("name"),
            slug: row.get("slug"),
            description: row.get("description"),
            label_id: row.get("label_id"),
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
    pub async fn update(self, pool: &PgPool) -> anyhow::Result<Self> {
        let slug = slugify(&self.name);
        let artist = sqlx::query_as::<_, Self>(
            "UPDATE artists SET name = $1, slug = $2, description = $3, updated_at = $4 WHERE id = $5 RETURNING *",
        )
        .bind(self.name)
        .bind(slug)
        .bind(self.description)
        .bind(chrono::Utc::now())
        .bind(self.id)
        .fetch_one(pool)
        .await
        .unwrap();

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

    #[cfg(feature = "ssr")]
    #[sqlx::test]
    async fn test_create(pool: PgPool) {
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = Artist::create(
            &pool,
            "Test Artist".to_string(),
            "This is a test artist".to_string(),
            record_label.id,
        )
        .await
        .unwrap();

        assert_eq!(artist.name, "Test Artist".to_string());
        assert_eq!(artist.description, "This is a test artist".to_string());
    }

    #[cfg(feature = "ssr")]
    #[sqlx::test]
    async fn test_get_by_slug(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let artist_by_slug = Artist::get_by_slug(&pool, artist.slug.clone())
            .await
            .unwrap();

        assert_eq!(artist, artist_by_slug);
    }

    #[cfg(feature = "ssr")]
    #[sqlx::test]
    async fn test_get_by_slug_not_found(pool: PgPool) {
        let artist = Artist::get_by_slug(&pool, "missing".to_string()).await;

        assert!(artist.is_err());
        assert_eq!(
            artist.unwrap_err().to_string(),
            "Could not find artist with slug missing".to_string()
        );
    }

    #[cfg(feature = "ssr")]
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

    #[cfg(feature = "ssr")]
    #[sqlx::test]
    async fn test_delete(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let result = artist.delete(&pool).await.unwrap();
        assert!(result.deleted_at.is_some());
    }

    #[cfg(feature = "ssr")]
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
