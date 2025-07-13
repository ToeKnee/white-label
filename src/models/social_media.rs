//! This module defines an enumeration for various social media platforms.
use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use sqlx::{FromRow, PgPool};
use std::fmt;

#[cfg(feature = "ssr")]
use super::artist::Artist;
use super::traits::Validate;

/// `SocialMedia` is an enum representing various social media platforms.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ssr", derive(sqlx::Type))]
pub enum SocialMedia {
    /// `BlueSky` is a decentralized social media platform.
    #[default]
    BlueSky,
    /// Facebook is a widely used social media platform.
    Facebook,
    /// Instagram is a photo and video sharing social media platform.
    Instagram,
    /// `LinkedIn` is a professional networking platform.
    LinkedIn,
    /// Mastodon is a decentralized social media platform.
    Mastodon,
    /// Pinterest is a visual discovery and bookmarking platform.
    Pinterest,
    /// Snapchat is a multimedia messaging app.
    Snapchat,
    /// Threads is a text-based social media platform by Meta.
    Threads,
    /// `TikTok` is a short-form video sharing platform.
    TikTok,
    /// Twitter is a microblogging and social networking service.
    Twitter,
    /// `YouTube` is a video sharing platform.
    YouTube,
}

impl fmt::Display for SocialMedia {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Represents a social media service associated with an artist.
#[derive(Serialize, Deserialize, Clone, Default, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "ssr", derive(FromRow))]
pub struct SocialMediaService {
    /// The unique identifier for the music service.
    pub id: i64,
    /// The unique identifier for the artist associated with the music service.
    pub artist_id: i64,
    /// The identifier for the social media.
    pub platform: SocialMedia,
    /// The URL of the atists page on the music service.
    pub url: String,
    /// The timestamp when the music service was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// The timestamp when the music service was last updated.
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl Validate for SocialMediaService {
    #[cfg(feature = "ssr")]
    async fn validate(&self, pool: &PgPool) -> anyhow::Result<()> {
        if self.artist_id <= 0 {
            return Err(anyhow::anyhow!(
                "Artist ID must be greater than 0".to_string()
            ));
        }
        if Artist::get_by_id(pool, self.artist_id).await.is_err() {
            return Err(anyhow::anyhow!("Artist not found".to_string()));
        }

        if self.url.is_empty() {
            return Err(anyhow::anyhow!("URL cannot be empty".to_string()));
        }
        Ok(())
    }
}

impl SocialMediaService {
    /// Creates a new social media service for an artist.
    ///
    /// # Arguments
    /// * `pool`: The database connection pool.
    /// * `artist_id`: The ID of the artist to associate with the social media service.
    /// * `platform`: The platform of the social media service.
    /// * `url`: The URL of the artist's page on the social media service.
    ///
    /// # Returns
    /// * A `SocialMediaService` instance representing the newly created social media service.
    ///
    /// # Errors
    /// If the artist does not exist, or if the URL is invalid, an error will be returned.
    #[cfg(feature = "ssr")]
    pub async fn create(
        pool: &PgPool,
        artist_id: i64,
        platform: SocialMedia,
        url: String,
    ) -> anyhow::Result<Self> {
        let service = Self {
            id: 0, // This will be set by the database
            artist_id,
            platform,
            url,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        service.validate(pool).await?;

        let service = sqlx::query_as::<_, Self>(
            "INSERT INTO social_media (artist_id, platform, url)
                VALUES ($1, $2, $3)
                RETURNING *",
        )
        .bind(service.artist_id)
        .bind(service.platform)
        .bind(service.url)
        .fetch_one(pool)
        .await?;

        Ok(service)
    }

    /// Lists all social media services associated with an artist.
    ///
    /// # Arguments
    /// * `pool`: The database connection pool.
    /// * `artist_id`: The ID of the artist whose social media services are to be listed.
    ///
    /// # Returns
    /// * A vector of `SocialMediaService` instances representing the social media services associated with the artist.
    ///
    /// # Errors
    /// If the artist does not exist, or if there is an issue with the database connection, an error will be returned.
    #[cfg(feature = "ssr")]
    pub async fn list_by_artist(pool: &PgPool, artist_id: i64) -> anyhow::Result<Vec<Self>> {
        if artist_id <= 0 {
            return Err(anyhow::anyhow!(
                "Artist ID must be greater than 0".to_string()
            ));
        }

        let services = sqlx::query_as::<_, Self>(
            "SELECT * FROM social_media WHERE artist_id = $1 ORDER BY created_at DESC",
        )
        .bind(artist_id)
        .fetch_all(pool)
        .await?;

        Ok(services)
    }

    /// Updates an existing social media service.
    ///
    /// # Arguments
    /// * `pool`: The database connection pool.
    ///
    /// # Returns
    /// * A `SocialMediaService` instance representing the updated social media service.
    ///
    /// # Errors
    /// If the social media service does not exist, or if the artist does not exist, an error will be returned.
    #[cfg(feature = "ssr")]
    pub async fn update(&self, pool: &PgPool) -> anyhow::Result<Self> {
        self.validate(pool).await?;

        let service = sqlx::query_as::<_, Self>(
            "UPDATE social_media SET artist_id = $1, platform = $2, url = $3, updated_at = NOW()
                WHERE id = $4 RETURNING *",
        )
        .bind(self.artist_id)
        .bind(self.platform.clone())
        .bind(self.url.clone())
        .bind(self.id)
        .fetch_one(pool)
        .await?;

        Ok(service)
    }

    /// Deletes a social media service.
    ///
    /// # Arguments
    /// * `pool`: The database connection pool.
    ///
    /// # Returns
    /// * A `Result` indicating success or failure.
    ///
    /// # Errors
    /// If the social media service does not exist, or if there is an issue with the database connection, an error will be returned.
    #[cfg(feature = "ssr")]
    pub async fn delete(&self, pool: &PgPool) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM social_media WHERE id = $1")
            .bind(self.id)
            .execute(pool)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "ssr")]
    use crate::models::test_helpers::create_test_artist;

    #[test]
    fn test_init_social_media_service() {
        let service = SocialMediaService {
            id: 0,
            artist_id: 1,
            platform: SocialMedia::BlueSky,
            url: "https://example.com".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        assert_eq!(service.id, 0);
        assert_eq!(service.artist_id, 1);
        assert_eq!(service.platform, SocialMedia::BlueSky);
        assert_eq!(service.url, "https://example.com".to_string());
    }

    #[sqlx::test]
    fn test_create_social_media_service(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let service = SocialMediaService::create(
            &pool,
            artist.id,
            SocialMedia::Facebook,
            "https://facebook.com/artist".to_string(),
        )
        .await
        .unwrap();

        assert_eq!(service.artist_id, artist.id);
        assert_eq!(service.platform, SocialMedia::Facebook);
        assert_eq!(service.url, "https://facebook.com/artist".to_string());
    }

    #[sqlx::test]
    fn test_list_social_media_by_artist(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let service1 = SocialMediaService::create(
            &pool,
            artist.id,
            SocialMedia::Facebook,
            "https://facebook.com/artist1".to_string(),
        )
        .await
        .unwrap();
        let service2 = SocialMediaService::create(
            &pool,
            artist.id,
            SocialMedia::Instagram,
            "https://instagram.com/artist2".to_string(),
        )
        .await
        .unwrap();

        let services = SocialMediaService::list_by_artist(&pool, artist.id)
            .await
            .unwrap();
        assert_eq!(services.len(), 2);
        assert!(services.contains(&service1));
        assert!(services.contains(&service2));
    }

    #[sqlx::test]
    fn test_update_social_media_service(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let mut service = SocialMediaService::create(
            &pool,
            artist.id,
            SocialMedia::Facebook,
            "https://facebook.com/artist".to_string(),
        )
        .await
        .unwrap();

        service.url = "https://facebook.com/updated_artist".to_string();
        let updated_service = service.update(&pool).await.unwrap();

        assert_eq!(updated_service.id, service.id);
        assert_eq!(updated_service.artist_id, artist.id);
        assert_eq!(updated_service.platform, SocialMedia::Facebook);
        assert_eq!(
            updated_service.url,
            "https://facebook.com/updated_artist".to_string()
        );
    }

    #[sqlx::test]
    fn test_delete_social_media_service(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let service = SocialMediaService::create(
            &pool,
            artist.id,
            SocialMedia::Facebook,
            "https://facebook.com/artist".to_string(),
        )
        .await
        .unwrap();

        service.delete(&pool).await.unwrap();

        let services = SocialMediaService::list_by_artist(&pool, artist.id)
            .await
            .unwrap();
        assert!(!services.contains(&service));
    }
}
