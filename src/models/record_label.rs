//! Label model
//!
//! The Label struct is used to represent a record label in the database.

use reactive_stores::Store;
use serde::{Deserialize, Serialize};

#[cfg(feature = "ssr")]
use crate::models::artist::Artist;

/// The Label struct is used to represent a record label in the database.
#[derive(Serialize, Deserialize, Clone, Default, Debug, Store, Eq, PartialEq)]
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
    // pub async fn create(name: String, description: String, isrc_base: String) -> Self {
    //     let slug = slugify(&name);
    // }

    /// Get a label by its slug
    #[cfg(feature = "ssr")]
    pub async fn first() -> anyhow::Result<Self> {
        use sqlx::Row;

        let row = sqlx::query("SELECT * FROM labels ORDER BY id ASC LIMIT 1")
            .fetch_one(crate::database::get_db())
            .await;

        let row = match row {
            Ok(row) => row,
            Err(e) => {
                eprintln!("{}", e);
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

    // /// Get a label by its slug
    // #[cfg(feature = "ssr")]
    // pub async fn get_by_slug(slug: String) -> anyhow::Result<Self> {
    //     use sqlx::Row;

    //     let row = sqlx::query("SELECT * FROM labels WHERE slug = $1")
    //         .bind(slug.clone())
    //         .fetch_one(crate::database::get_db())
    //         .await;

    //     let row = match row {
    //         Ok(row) => row,
    //         Err(e) => {
    //             eprintln!("{}", e);
    //             return Err(anyhow::anyhow!("Could not find label with slug {}", slug));
    //         }
    //     };

    //     Ok(Self {
    //         id: row.get("id"),
    //         name: row.get("name"),
    //         slug: row.get("slug"),
    //         description: row.get("description"),
    //         isrc_base: row.get("isrc_base"),
    //         created_at: row.get("created_at"),
    //         updated_at: row.get("updated_at"),
    //     })
    // }

    // #[cfg(feature = "ssr")]
    // pub async fn update() -> Self {}

    // #[cfg(feature = "ssr")]
    // pub async fn delete() -> Self {}

    #[cfg(feature = "ssr")]
    pub async fn artists(self) -> anyhow::Result<Vec<Artist>> {
        use sqlx::Row;

        let rows = sqlx::query("SELECT * FROM artists WHERE label_id = $1 ORDER BY name ASC")
            .bind(self.id)
            .fetch_all(crate::database::get_db())
            .await;

        let rows = match rows {
            Ok(rows) => rows,
            Err(e) => {
                eprintln!("{}", e);
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
