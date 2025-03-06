//! Page model
//!
//! The Page struct is used to represent a record page in the database.

use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use sqlx::{FromRow, PgPool, Row};

#[cfg(feature = "ssr")]
use super::record_label::RecordLabel;
use super::traits::Validate;
#[cfg(feature = "ssr")]
use crate::utils::slugify::slugify;

/// The Page struct is used to represent a page in the database.
#[derive(Serialize, Deserialize, Clone, Default, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "ssr", derive(FromRow))]
pub struct Page {
    /// The unique identifier of the page
    pub id: i64,
    /// The name of the page
    pub name: String,
    /// The slug of the page
    pub slug: String,
    /// The meta description of the page
    pub description: String,
    /// The content of the page in markdown
    pub body: String,
    /// The label id
    pub label_id: i64,
    /// The date the page is published.
    /// If this is None, the page is not published
    /// If this is in the future, the page is scheduled to be published
    /// If this is in the past, the page is published
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
    /// The date and time the page was created in the database
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// The date and time the page was last updated
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// The date and time the page was deleted
    /// If this is None, the page is not deleted
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl Validate for Page {
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
        if let Ok(page) = Self::get_by_slug(pool, self.slug.clone()).await {
            if page.id != self.id {
                return Err(anyhow::anyhow!("Slug must be unique.".to_string()));
            }
        }

        // Check that the description is less than 255 characters
        if self.description.len() > 255 {
            return Err(anyhow::anyhow!(
                "Description must be less than 255 characters.".to_string()
            ));
        }

        // Check that the record label exists
        if let Err(e) = RecordLabel::get_by_id(pool, self.label_id).await {
            leptos::logging::error!("{e}");
            return Err(anyhow::anyhow!(
                "Record Label with id {} does not exist.",
                self.label_id
            ));
        }

        Ok(())
    }
}

impl Page {
    /// Create a new page
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `name` - The name of the page
    /// * `description` - The description of the page
    /// * `record_label_id` - The ID of the record label the page is signed to
    ///
    /// # Returns
    /// The created page
    ///
    /// # Errors
    /// If the page cannot be created, return an error
    /// If the record label is not found, return an error
    #[cfg(feature = "ssr")]
    pub async fn create(
        pool: &PgPool,
        name: String,
        description: String,
        body: String,
        record_label_id: i64,
        published_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> anyhow::Result<Self> {
        let slug = slugify(&name);

        let page = Self {
            id: 0,
            name,
            slug,
            description,
            body,
            label_id: record_label_id,
            published_at,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };
        page.validate(pool).await?;

        let page = sqlx::query_as::<_, Self>(
         "INSERT INTO pages (name, slug, description, body, label_id, published_at) VALUES ($1, $2, $3, $4, $5, $6) RETURNING *",
     )
         .bind(page.name)
         .bind(page.slug)
         .bind(page.description)
         .bind(page.body)
         .bind(page.label_id)
         .bind(page.published_at)
         .fetch_one(pool)
         .await?;

        Ok(page)
    }

    /// Get page by slug
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `slug` - The slug of the page
    ///
    /// # Returns
    /// The page
    ///
    /// # Errors
    /// If the page cannot be found, return an error
    #[cfg(feature = "ssr")]
    pub async fn get_by_slug(pool: &PgPool, slug: String) -> anyhow::Result<Self> {
        let row = sqlx::query("SELECT * FROM pages WHERE slug = $1")
            .bind(slug.clone())
            .fetch_one(pool)
            .await;

        let row = match row {
            Ok(row) => row,
            Err(e) => {
                leptos::logging::error!("{e}");
                return Err(anyhow::anyhow!("Could not find page with slug {}.", slug));
            }
        };

        Ok(Self {
            id: row.get("id"),
            name: row.get("name"),
            slug: row.get("slug"),
            description: row.get("description"),
            body: row.get("body"),
            label_id: row.get("label_id"),
            published_at: row.get("published_at"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            deleted_at: row.get("deleted_at"),
        })
    }

    /// Update an page
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    ///
    /// # Returns
    /// The updated page
    ///
    /// # Errors
    /// If the page cannot be updated, return an error
    ///
    /// # Panics
    /// If the page cannot be updated, return an error
    #[cfg(feature = "ssr")]
    pub async fn update(mut self, pool: &PgPool) -> anyhow::Result<Self> {
        self.slug = slugify(&self.name);
        self.validate(pool).await?;

        let page = match sqlx::query_as::<_, Self>(
            "UPDATE pages SET name = $1, slug = $2, description = $3, body = $4, published_at = $5, updated_at = $6 WHERE id = $7 RETURNING *",
        )
        .bind(self.name)
        .bind(self.slug)
        .bind(self.description)
        .bind(self.body)
        .bind(self.published_at)
        .bind(chrono::Utc::now())
        .bind(self.id)
        .fetch_one(pool)
        .await {
            Ok(page) => page,
            Err(e) => {
                leptos::logging::error!("{e}");
                return Err(anyhow::anyhow!(
                    "Could not update page with id {}.",
                    self.id
                ));
            }
        };

        Ok(page)
    }

    /// Delete an page
    /// This is a soft delete
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    ///
    /// # Returns
    /// The deleted page
    ///
    /// # Errors
    /// If the page cannot be deleted, return an error
    #[cfg(feature = "ssr")]
    pub async fn delete(&self, pool: &PgPool) -> anyhow::Result<Self> {
        let page =
            sqlx::query_as::<_, Self>("UPDATE pages SET deleted_at = $1 WHERE id = $2 RETURNING *")
                .bind(chrono::Utc::now())
                .bind(self.id)
                .fetch_one(pool)
                .await;

        match page {
            Ok(page) => Ok(page),
            Err(e) => {
                eprintln!("{e}");
                Err(anyhow::anyhow!(
                    "Could not delete page with id {}.",
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
    use crate::models::test_helpers::{create_test_page, create_test_record_label};

    #[test]
    fn test_init_page() {
        let page = Page {
            id: 1,
            name: "Test Page".to_string(),
            slug: "test-page".to_string(),
            description: "This is a test page".to_string(),
            body: "This is a test page".to_string(),
            label_id: 1,
            published_at: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        assert_eq!(page.id, 1);
        assert_eq!(page.name, "Test Page".to_string());
        assert_eq!(page.slug, "test-page".to_string());
        assert_eq!(page.description, "This is a test page".to_string());
        assert_eq!(page.label_id, 1);
    }

    #[sqlx::test]
    async fn test_validate_success(pool: PgPool) {
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let page = Page {
            id: 1,
            name: "Test Page".to_string(),
            slug: "test-page".to_string(),
            description: "This is a test page".to_string(),
            body: "This is a test page".to_string(),
            label_id: record_label.id,
            published_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let result = page.validate(&pool).await;

        assert!(result.is_ok());
    }

    #[sqlx::test]
    async fn test_validate_name_is_empty(pool: PgPool) {
        let page = Page {
            id: 1,
            name: String::new(),
            slug: "test-page".to_string(),
            description: "This is a test page".to_string(),
            body: "This is a test page".to_string(),
            label_id: 1,
            published_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let result = page.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Name is required.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_validate_name_length(pool: PgPool) {
        let name = "a".repeat(256);
        let page = Page {
            id: 1,
            name,
            slug: "test-page".to_string(),
            description: "This is a test page".to_string(),
            body: "This is a test page".to_string(),
            label_id: 1,
            published_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let result = page.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Name must be less than 255 characters.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_validate_slug_length(pool: PgPool) {
        let slug = "a".repeat(256);
        let page = Page {
            id: 1,
            name: "Test Page".to_string(),
            slug,
            description: "This is a test page".to_string(),
            body: "This is a test page".to_string(),
            label_id: 1,
            published_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let result = page.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Slug must be less than 255 characters.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_validate_slug_unique(pool: PgPool) {
        let page = create_test_page(&pool, 1, None).await.unwrap();
        let mut new_page = page.clone();
        new_page.id = 2;
        new_page.slug = page.slug.clone();

        let result = new_page.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Slug must be unique.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_validate_description_length(pool: PgPool) {
        let description = "a".repeat(256);
        let page = Page {
            id: 1,
            name: "Test Page".to_string(),
            slug: "test-page".to_string(),
            description,
            body: "This is a test page".to_string(),
            label_id: 1,
            published_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let result = page.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Description must be less than 255 characters.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_validate_record_label_exists(pool: PgPool) {
        let page = Page {
            id: 1,
            name: "Test Page".to_string(),
            slug: "test-page".to_string(),
            description: "This is a test page".to_string(),
            body: "This is a test page".to_string(),
            label_id: 1,
            published_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        };

        let result = page.validate(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Record Label with id 1 does not exist.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_create(pool: PgPool) {
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let page = Page::create(
            &pool,
            "Test Page".to_string(),
            "This is a test page".to_string(),
            "This is a test page body".to_string(),
            record_label.id,
            Some(chrono::Utc::now()),
        )
        .await
        .unwrap();

        assert_eq!(page.name, "Test Page".to_string());
        assert_eq!(page.slug, "test-page".to_string());
        assert_eq!(page.description, "This is a test page".to_string());
        assert_eq!(page.body, "This is a test page body".to_string());
    }

    #[sqlx::test]
    async fn test_create_with_validation_error(pool: PgPool) {
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let page = Page::create(
            &pool,
            String::new(),
            "This is a test page".to_string(),
            "This is a test page body".to_string(),
            record_label.id,
            Some(chrono::Utc::now()),
        )
        .await;

        assert!(page.is_err());
        assert_eq!(
            page.unwrap_err().to_string(),
            "Name is required.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_get_by_slug(pool: PgPool) {
        let page = create_test_page(&pool, 1, None).await.unwrap();
        let page_by_slug = Page::get_by_slug(&pool, page.slug.clone()).await.unwrap();

        assert_eq!(page, page_by_slug);
    }

    #[sqlx::test]
    async fn test_get_by_slug_not_found(pool: PgPool) {
        let page = Page::get_by_slug(&pool, "missing".to_string()).await;

        assert!(page.is_err());
        assert_eq!(
            page.unwrap_err().to_string(),
            "Could not find page with slug missing.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_update(pool: PgPool) {
        let page = create_test_page(&pool, 1, None).await.unwrap();
        let mut update_page = page.clone();
        update_page.name = "Updated Page".to_string();
        update_page.description = "This is an updated page".to_string();
        update_page.body = "This is an updated page body".to_string();

        let updated_page = update_page.update(&pool).await.unwrap();
        assert_eq!(updated_page.name, "Updated Page".to_string());
        assert_eq!(updated_page.slug, "updated-page".to_string());
        assert_eq!(
            updated_page.description,
            "This is an updated page".to_string()
        );
        assert_eq!(
            updated_page.body,
            "This is an updated page body".to_string()
        );
        assert_ne!(updated_page.updated_at, page.updated_at);
    }

    #[sqlx::test]
    async fn test_update_validation_error(pool: PgPool) {
        let page = create_test_page(&pool, 1, None).await.unwrap();
        let mut update_page = page.clone();
        update_page.name = String::new();
        let updated_page = update_page.update(&pool).await;

        assert!(updated_page.is_err());
        assert_eq!(
            updated_page.unwrap_err().to_string(),
            "Name is required.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_delete(pool: PgPool) {
        let page = create_test_page(&pool, 1, None).await.unwrap();
        let result = page.delete(&pool).await.unwrap();
        assert!(result.deleted_at.is_some());
    }

    #[sqlx::test]
    async fn test_delete_not_found(pool: PgPool) {
        let page = Page::default();
        let result = page.delete(&pool).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Could not delete page with id 0.".to_string()
        );
    }
}
