//! Label model
//!
//! The Label struct is used to represent a record label in the database.

use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use sqlx::{FromRow, PgPool, Row};

#[cfg(feature = "ssr")]
use super::artist::Artist;
use super::traits::Validate;
#[cfg(feature = "ssr")]
use crate::utils::slugify::slugify;

/// The Label struct is used to represent a record label in the database.
#[derive(Serialize, Deserialize, Clone, Default, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "ssr", derive(FromRow))]
pub struct RecordLabel {
    /// The unique identifier of the label
    pub id: i64,
    /// The name of the label
    pub name: String,
    /// The slug of the label
    pub slug: String,
    /// The description of the label
    pub description: String,
    /// The ISRC base of the label
    pub isrc_base: String,
    /// The date and time the label was created in the database
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// The date and time the label was last updated
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl Validate for RecordLabel {
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
        if let Ok(record_label) = Self::get_by_slug(pool, self.slug.clone()).await {
            if record_label.id != self.id {
                return Err(anyhow::anyhow!("Slug must be unique.".to_string()));
            }
        }

        Ok(())
    }
}

impl RecordLabel {
    /// Get first record label
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    ///
    /// # Returns
    /// The record label
    ///
    /// # Errors
    /// If the record label cannot be found, return an error
    #[cfg(feature = "ssr")]
    pub async fn first(pool: &PgPool) -> anyhow::Result<Self> {
        let row = sqlx::query("SELECT * FROM labels ORDER BY id ASC LIMIT 1")
            .fetch_one(pool)
            .await;

        let row = match row {
            Ok(row) => row,
            Err(e) => {
                leptos::logging::error!("{e}");
                return Err(anyhow::anyhow!("Could not find label"));
            }
        };

        Ok(Self {
            id: row.get("id"),
            name: row.get("name"),
            slug: row.get("slug"),
            description: row.get("description"),
            isrc_base: row.get("isrc_base"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    /// Get a record label by id
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `id` - The ID of the record label
    ///
    /// # Returns
    /// The record label
    ///
    /// # Errors
    /// If the record label cannot be found, return an error
    #[cfg(feature = "ssr")]
    pub async fn get_by_id(pool: &PgPool, id: i64) -> anyhow::Result<Self> {
        let row = sqlx::query("SELECT * FROM labels WHERE id = $1")
            .bind(id)
            .fetch_one(pool)
            .await;

        let row = match row {
            Ok(row) => row,
            Err(e) => {
                leptos::logging::error!("{e}");
                return Err(anyhow::anyhow!(
                    "Could not find record label with id {}.",
                    id
                ));
            }
        };

        Ok(Self {
            id: row.get("id"),
            name: row.get("name"),
            slug: row.get("slug"),
            description: row.get("description"),
            isrc_base: row.get("isrc_base"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    /// Get a record label by slug
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `slug` - The slug of the record label
    ///
    /// # Returns
    /// The record label
    ///
    /// # Errors
    /// If the record label cannot be found, return an error
    #[cfg(feature = "ssr")]
    pub async fn get_by_slug(pool: &PgPool, slug: String) -> anyhow::Result<Self> {
        let row = sqlx::query("SELECT * FROM labels WHERE slug = $1")
            .bind(&slug)
            .fetch_one(pool)
            .await;

        let row = match row {
            Ok(row) => row,
            Err(e) => {
                leptos::logging::error!("{e}");
                return Err(anyhow::anyhow!(
                    "Could not find record label with slug {}.",
                    slug
                ));
            }
        };

        Ok(Self {
            id: row.get("id"),
            name: row.get("name"),
            slug: row.get("slug"),
            description: row.get("description"),
            isrc_base: row.get("isrc_base"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    /// Update a label
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    ///
    /// # Returns
    /// The updated label
    ///
    /// # Errors
    /// If the label cannot be updated, return an error
    #[cfg(feature = "ssr")]
    pub async fn update(mut self, pool: &PgPool) -> anyhow::Result<Self> {
        self.slug = slugify(&self.name);
        self.validate(pool).await?;

        let row = sqlx::query("UPDATE labels SET name = $1, slug=$2, description = $3, isrc_base = $4, updated_at = NOW() WHERE id = $5 RETURNING *")
            .bind(self.name)
            .bind(self.slug)
            .bind(self.description)
            .bind(self.isrc_base)
            .bind(self.id)
            .fetch_one(pool)
            .await;

        let row = match row {
            Ok(row) => row,
            Err(e) => {
                leptos::logging::error!("{e}");
                return Err(anyhow::anyhow!("Could not update label."));
            }
        };

        Ok(Self {
            id: row.get("id"),
            name: row.get("name"),
            slug: row.get("slug"),
            description: row.get("description"),
            isrc_base: row.get("isrc_base"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    /// Get a label by its slug
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    ///
    /// # Returns
    /// The artists signed to the label
    ///
    /// # Errors
    /// If the artists cannot be retrieved, return an error
    #[cfg(feature = "ssr")]
    pub async fn artists(self, pool: &PgPool, include_hidden: bool) -> anyhow::Result<Vec<Artist>> {
        let query = if include_hidden {
            "SELECT *
             FROM artists
             WHERE label_id = $1
             ORDER BY deleted_at DESC, name ASC"
        } else {
            "SELECT *
            FROM artists
            WHERE label_id = $1
              AND deleted_at IS NULL
              AND published_at < NOW()
              AND published_at IS NOT NULL
            ORDER BY name ASC"
        };

        let artists = sqlx::query_as::<_, Artist>(query)
            .bind(self.id)
            //.bind(chrono::Utc::now())
            .fetch_all(pool)
            .await;

        match artists {
            Ok(artists) => Ok(artists),
            Err(e) => {
                eprintln!("{e}");
                Err(anyhow::anyhow!("Could not find artists"))
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
    fn test_init() {
        let record_label = RecordLabel {
            id: 1,
            name: "Test Label".to_string(),
            slug: "test-label".to_string(),
            description: "This is a test label".to_string(),
            isrc_base: "UK ABC".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        assert_eq!(record_label.id, 1);
        assert_eq!(record_label.name, "Test Label".to_string());
        assert_eq!(record_label.slug, "test-label".to_string());
        assert_eq!(record_label.description, "This is a test label".to_string());
        assert_eq!(record_label.isrc_base, "UK ABC".to_string());
    }

    #[sqlx::test]
    async fn test_validate_success(pool: PgPool) {
        let record_label = RecordLabel {
            id: 1,
            name: "Test Record Label".to_string(),
            slug: "test-record-label".to_string(),
            description: "This is a test record label".to_string(),
            isrc_base: "UK ABC".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let result = record_label.validate(&pool).await;

        assert!(result.is_ok());
    }

    #[sqlx::test]
    async fn test_validate_name_is_empty(pool: PgPool) {
        let record_label = RecordLabel {
            id: 1,
            name: String::new(),
            slug: "test-record-label".to_string(),
            description: "This is a test record label".to_string(),
            isrc_base: "UK ABC".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let result = record_label.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Name is required.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_validate_name_length(pool: PgPool) {
        let name = "a".repeat(256);
        let record_label = RecordLabel {
            id: 1,
            name,
            slug: "test-record-label".to_string(),
            description: "This is a test record label".to_string(),
            isrc_base: "UK ABC".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let result = record_label.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Name must be less than 255 characters.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_validate_slug_length(pool: PgPool) {
        let slug = "a".repeat(256);
        let record_label = RecordLabel {
            id: 1,
            name: "Test Record Label".to_string(),
            slug,
            description: "This is a test record label".to_string(),
            isrc_base: "UK ABC".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let result = record_label.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Slug must be less than 255 characters.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_validate_slug_unique(pool: PgPool) {
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let mut new_record_label = record_label.clone();
        new_record_label.id = 2;
        new_record_label.slug = record_label.slug.clone();

        let result = new_record_label.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Slug must be unique.".to_string()
        );
    }

    #[cfg(feature = "ssr")]
    #[sqlx::test]
    async fn test_get_first_with_no_rows_in_db(pool: PgPool) {
        let record_label = RecordLabel::first(&pool).await;
        assert!(record_label.is_err());
        assert_eq!(
            record_label.unwrap_err().to_string(),
            "Could not find label".to_string()
        );
    }

    #[cfg(feature = "ssr")]
    #[sqlx::test]
    async fn test_get_first(pool: PgPool) {
        let test_label = create_test_record_label(&pool, 1).await.unwrap();
        let record_label = RecordLabel::first(&pool).await.unwrap();
        assert_eq!(record_label.id, 1);
        assert_eq!(record_label.name, test_label.name);
        assert_eq!(record_label.slug, test_label.slug);
        assert_eq!(record_label.description, test_label.description);
        assert_eq!(record_label.isrc_base, test_label.isrc_base);
    }

    #[cfg(feature = "ssr")]
    #[sqlx::test]
    async fn test_get_by_id_no_label(pool: PgPool) {
        let record_label = RecordLabel::get_by_id(&pool, 1).await;
        assert!(record_label.is_err());
        assert_eq!(
            record_label.unwrap_err().to_string(),
            "Could not find record label with id 1.".to_string()
        );
    }

    #[cfg(feature = "ssr")]
    #[sqlx::test]
    async fn test_get_by_id(pool: PgPool) {
        let test_label = create_test_record_label(&pool, 1).await.unwrap();
        let record_label = RecordLabel::get_by_id(&pool, test_label.id).await.unwrap();
        assert_eq!(record_label.id, 1);
        assert_eq!(record_label.name, test_label.name);
        assert_eq!(record_label.slug, test_label.slug);
        assert_eq!(record_label.description, test_label.description);
        assert_eq!(record_label.isrc_base, test_label.isrc_base);
    }

    #[cfg(feature = "ssr")]
    #[sqlx::test]
    async fn test_get_by_slug_no_label(pool: PgPool) {
        let record_label = RecordLabel::get_by_slug(&pool, "missing".to_string()).await;
        assert!(record_label.is_err());
        assert_eq!(
            record_label.unwrap_err().to_string(),
            "Could not find record label with slug missing.".to_string()
        );
    }

    #[cfg(feature = "ssr")]
    #[sqlx::test]
    async fn test_get_by_slug(pool: PgPool) {
        let test_label = create_test_record_label(&pool, 1).await.unwrap();
        let record_label = RecordLabel::get_by_slug(&pool, test_label.slug.clone())
            .await
            .unwrap();
        assert_eq!(record_label.id, test_label.id);
        assert_eq!(record_label.name, test_label.name);
        assert_eq!(record_label.slug, test_label.slug);
        assert_eq!(record_label.description, test_label.description);
        assert_eq!(record_label.isrc_base, test_label.isrc_base);
    }

    #[cfg(feature = "ssr")]
    #[sqlx::test]
    async fn test_update_label(pool: PgPool) {
        let mut record_label = create_test_record_label(&pool, 1).await.unwrap();
        record_label.name = "Updated Label".to_string();
        record_label.description = "This is an updated label".to_string();
        record_label.isrc_base = "UK XYZ".to_string();
        let updated_label = record_label.update(&pool).await.unwrap();
        assert_eq!(updated_label.id, 1);
        assert_eq!(updated_label.name, "Updated Label".to_string());
        assert_eq!(updated_label.slug, "updated-label".to_string());
        assert_eq!(
            updated_label.description,
            "This is an updated label".to_string()
        );
        assert_eq!(updated_label.isrc_base, "UK XYZ".to_string());
    }

    #[cfg(feature = "ssr")]
    #[sqlx::test]
    async fn test_update_label_change_id(pool: PgPool) {
        let mut record_label = create_test_record_label(&pool, 1).await.unwrap();
        record_label.id = 2;
        record_label.name = "Updated Label".to_string();
        let updated_label = record_label.update(&pool).await;
        assert!(updated_label.is_err());
        assert_eq!(
            updated_label.unwrap_err().to_string(),
            "Could not update label.".to_string()
        );
    }

    #[cfg(feature = "ssr")]
    #[sqlx::test]
    async fn test_artists_hide_artists(pool: PgPool) {
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let mut unpublished_artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();
        unpublished_artist.published_at = None;
        unpublished_artist.clone().update(&pool).await.unwrap();
        let mut published_artist = create_test_artist(&pool, 2, Some(record_label.clone()))
            .await
            .unwrap();
        published_artist.published_at = Some(chrono::Utc::now() - chrono::Duration::days(1));
        let published_artist = published_artist.clone().update(&pool).await.unwrap();
        let mut future_artist = create_test_artist(&pool, 3, Some(record_label.clone()))
            .await
            .unwrap();
        future_artist.published_at = Some(chrono::Utc::now() + chrono::Duration::days(1));
        future_artist.clone().update(&pool).await.unwrap();
        let deleted_artist = create_test_artist(&pool, 4, Some(record_label.clone()))
            .await
            .unwrap();
        deleted_artist.clone().delete(&pool).await.unwrap();
        let artists = record_label.artists(&pool, false).await.unwrap();
        assert_eq!(artists, vec![published_artist]);
    }

    #[cfg(feature = "ssr")]
    #[sqlx::test]
    async fn test_artists_include_hidden_artists(pool: PgPool) {
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let mut unpublished_artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();
        unpublished_artist.published_at = None;
        let unpublished_artist = unpublished_artist.clone().update(&pool).await.unwrap();
        let mut published_artist = create_test_artist(&pool, 2, Some(record_label.clone()))
            .await
            .unwrap();
        published_artist.published_at = Some(chrono::Utc::now() - chrono::Duration::days(1));
        let published_artist = published_artist.clone().update(&pool).await.unwrap();
        let mut future_artist = create_test_artist(&pool, 3, Some(record_label.clone()))
            .await
            .unwrap();
        future_artist.published_at = Some(chrono::Utc::now() + chrono::Duration::days(1));
        let future_artist = future_artist.clone().update(&pool).await.unwrap();
        let deleted_artist = create_test_artist(&pool, 4, Some(record_label.clone()))
            .await
            .unwrap();
        let deleted_artist = deleted_artist.clone().delete(&pool).await.unwrap();
        let artists = record_label.artists(&pool, true).await.unwrap();
        assert_eq!(
            artists,
            vec![
                unpublished_artist,
                published_artist,
                future_artist,
                deleted_artist,
            ]
        );
    }
}
