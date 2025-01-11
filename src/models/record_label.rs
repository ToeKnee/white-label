//! Label model
//!
//! The Label struct is used to represent a record label in the database.

use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use sqlx::{FromRow, PgPool, Row};

#[cfg(feature = "ssr")]
use crate::models::artist::Artist;

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

impl RecordLabel {
    /// Get a label by its slug
    #[cfg(feature = "ssr")]
    pub async fn first(pool: &PgPool) -> anyhow::Result<Self> {
        let row = sqlx::query("SELECT * FROM labels ORDER BY id ASC LIMIT 1")
            .fetch_one(pool)
            .await;

        let row = match row {
            Ok(row) => row,
            Err(e) => {
                eprintln!("{e}");
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

    /// Get a label by its slug
    #[cfg(feature = "ssr")]
    pub async fn get_by_id(pool: &PgPool, id: i64) -> anyhow::Result<Self> {
        let row = sqlx::query("SELECT * FROM labels WHERE id = $1")
            .bind(id)
            .fetch_one(pool)
            .await;

        let row = match row {
            Ok(row) => row,
            Err(e) => {
                eprintln!("{e}");
                return Err(anyhow::anyhow!("Could not find label with id {}", id));
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

    #[cfg(feature = "ssr")]
    pub async fn update(self, pool: &PgPool) -> anyhow::Result<Self> {
        use crate::utils::slugify::slugify;

        let slug = slugify(&self.name);
        let row = sqlx::query("UPDATE labels SET name = $1, slug=$2, description = $3, isrc_base = $4, updated_at = NOW() WHERE id = $5 RETURNING *")
            .bind(self.name)
            .bind(slug)
            .bind(self.description)
            .bind(self.isrc_base)
            .bind(self.id)
            .fetch_one(pool)
            .await;

        let row = match row {
            Ok(row) => row,
            Err(e) => {
                eprintln!("{e}");
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

    #[cfg(feature = "ssr")]
    pub async fn artists(self, pool: &PgPool) -> anyhow::Result<Vec<Artist>> {
        let rows = sqlx::query("SELECT * FROM artists WHERE label_id = $1 ORDER BY name ASC")
            .bind(self.id)
            .fetch_all(pool)
            .await;

        let rows = match rows {
            Ok(rows) => rows,
            Err(e) => {
                eprintln!("{e}");
                return Err(anyhow::anyhow!("Could not find artists"));
            }
        };

        let mut artists = Vec::new();
        for row in rows {
            artists.push(Artist {
                id: row.get("id"),
                name: row.get("name"),
                slug: row.get("slug"),
                description: row.get("description"),
                label_id: row.get("label_id"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            });
        }

        Ok(artists)
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
            "Could not find label with id 1".to_string()
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
        let updated_label = record_label.update(&pool).await;
        assert!(updated_label.is_err());
        assert_eq!(
            updated_label.unwrap_err().to_string(),
            "Could not update label.".to_string()
        );
    }

    #[cfg(feature = "ssr")]
    #[sqlx::test]
    async fn test_artists(pool: PgPool) {
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();
        let artists = record_label.artists(&pool).await.unwrap();
        assert_eq!(artists.len(), 1);
        assert_eq!(artists[0], artist);
    }
}
