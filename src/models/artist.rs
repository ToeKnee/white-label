//! Artist model
//!
//! The Artist struct is used to represent a record artist in the database.

use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use sqlx::{FromRow, PgPool, Row};

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
}

impl Artist {
    // pub async fn create(name: String, description: String, isrc_base: String) -> Self {
    //     let slug = slugify(&name);
    // }

    /// Get a artist by its slug
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
        })
    }

    // #[cfg(feature = "ssr")]
    // pub async fn update() -> Self {}

    // #[cfg(feature = "ssr")]
    // pub async fn delete() -> Self {}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "ssr")]
    use crate::models::test_helpers::create_test_artist;

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
        };

        assert_eq!(artist.id, 1);
        assert_eq!(artist.name, "Test Artist".to_string());
        assert_eq!(artist.slug, "test-artist".to_string());
        assert_eq!(artist.description, "This is a test artist".to_string());
        assert_eq!(artist.label_id, 1);
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
        create_test_artist(&pool, 1, None).await.unwrap();
        let artist = Artist::get_by_slug(&pool, "missing".to_string()).await;

        assert!(artist.is_err());
        assert_eq!(
            artist.unwrap_err().to_string(),
            "Could not find artist with slug missing".to_string()
        );
    }
}
