//! Artist model
//!
//! The Artist struct is used to represent a record artist in the database.

use reactive_stores::Store;
use serde::{Deserialize, Serialize};

/// The Artist struct is used to represent a record artist in the database.
#[derive(Serialize, Deserialize, Clone, Default, Debug, Store, Eq, PartialEq)]
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
    pub async fn get_by_slug(slug: String) -> anyhow::Result<Self> {
        use sqlx::Row;

        let row = sqlx::query("SELECT * FROM artists WHERE slug = $1")
            .bind(slug.clone())
            .fetch_one(crate::database::get_db())
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
