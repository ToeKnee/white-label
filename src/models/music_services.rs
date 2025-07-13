//! This module defines varous music services

use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use sqlx::{FromRow, PgPool};
use std::fmt;

#[cfg(feature = "ssr")]
use super::artist::Artist;
use super::traits::Validate;

/// Enum representing different music services.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ssr", derive(sqlx::Type))]
pub enum Platform {
    /// Amazon Music service.
    /// Amazon Music is a music streaming platform and online music store operated by Amazon.
    AmazonMusic,
    /// Apple Music service.
    /// Apple Music is a music streaming service developed by Apple Inc.
    AppleMusic,
    /// Bandcamp music service.
    /// Bandcamp is a platform for independent musicians to share and sell their music.
    #[default]
    Bandcamp,
    /// Beatport music service.
    /// This service is primarily used for electronic music and DJ tracks.
    Beatport,
    /// Deezer music service.
    /// Deezer is a music streaming service that offers a wide range of music tracks.
    Deezer,
    /// `SoundCloud` music service.
    /// `SoundCloud` is a platform for sharing and discovering music.
    SoundCloud,
    /// Spotify music service.
    /// Spotify is a popular music streaming service that provides access to a vast library of songs.
    Spotify,
    /// Tidal music service.
    /// Tidal is a subscription-based music streaming service known for its high-fidelity sound quality.
    Tidal,
    /// `YouTube` Music service.
    /// `YouTube` Music is a music streaming service developed by `YouTube`, a subsidiary of Google.
    YouTubeMusic,
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Represents a music service associated with an artist.
#[derive(Serialize, Deserialize, Clone, Default, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "ssr", derive(FromRow))]
pub struct MusicService {
    /// The unique identifier for the music service.
    pub id: i64,
    /// The unique identifier for the artist associated with the music service.
    pub artist_id: i64,
    /// The identifier for the music service.
    pub platform: Platform,
    /// The URL of the atists page on the music service.
    pub url: String,
    /// The timestamp when the music service was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// The timestamp when the music service was last updated.
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl Validate for MusicService {
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

impl MusicService {
    /// Creates a new music service for an artist.
    ///
    /// # Arguments
    /// * `pool`: The database connection pool.
    /// * `artist_id`: The ID of the artist to associate with the music service.
    /// * `platform`: The platform of the music service.
    /// * `url`: The URL of the artist's page on the music service.
    ///
    /// # Returns
    /// * A `MusicService` instance representing the newly created music service.
    ///
    /// # Errors
    /// If the artist does not exist, or if the URL is invalid, an error will be returned.
    #[cfg(feature = "ssr")]
    pub async fn create(
        pool: &PgPool,
        artist_id: i64,
        platform: Platform,
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
            "INSERT INTO music_services (artist_id,platform, url)
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

    /// Lists all music services associated with an artist.
    ///
    /// # Arguments
    /// * `pool`: The database connection pool.
    /// * `artist_id`: The ID of the artist whose music services are to be listed.
    ///
    /// # Returns
    /// * A vector of `MusicService` instances representing the music services associated with the artist.
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
            "SELECT * FROM music_services WHERE artist_id = $1 ORDER BY created_at DESC",
        )
        .bind(artist_id)
        .fetch_all(pool)
        .await?;

        Ok(services)
    }

    /// Updates an existing music service.
    ///
    /// # Arguments
    /// * `pool`: The database connection pool.
    ///
    /// # Returns
    /// * A `MusicService` instance representing the updated music service.
    ///
    /// # Errors
    /// If the music service does not exist, or if the artist does not exist, an error will be returned.
    #[cfg(feature = "ssr")]
    pub async fn update(&self, pool: &PgPool) -> anyhow::Result<Self> {
        self.validate(pool).await?;

        let service = sqlx::query_as::<_, Self>(
            "UPDATE music_services SET artist_id = $1, platform = $2, url = $3, updated_at = NOW()
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

    /// Deletes a music service.
    ///
    /// # Arguments
    /// * `pool`: The database connection pool.
    ///
    /// # Returns
    /// * A `Result` indicating success or failure.
    ///
    /// # Errors
    /// If the music service does not exist, or if there is an issue with the database connection, an error will be returned.
    #[cfg(feature = "ssr")]
    pub async fn delete(&self, pool: &PgPool) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM music_services WHERE id = $1")
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
    fn test_init_music_service() {
        let service = MusicService {
            id: 0,
            artist_id: 1,
            platform: Platform::Bandcamp,
            url: "https:://example.com".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        assert_eq!(service.id, 0);
        assert_eq!(service.artist_id, 1);
        assert_eq!(service.platform, Platform::Bandcamp);
        assert_eq!(service.url, "https:://example.com".to_string());
    }

    #[sqlx::test]
    fn test_create_music_service(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let service = MusicService::create(
            &pool,
            artist.id,
            Platform::Spotify,
            "https://spotify.com/artist".to_string(),
        )
        .await
        .unwrap();

        assert_eq!(service.artist_id, artist.id);
        assert_eq!(service.platform, Platform::Spotify);
        assert_eq!(service.url, "https://spotify.com/artist".to_string());
    }

    #[sqlx::test]
    fn test_list_music_services_by_artist(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let service1 = MusicService::create(
            &pool,
            artist.id,
            Platform::Spotify,
            "https://spotify.com/artist1".to_string(),
        )
        .await
        .unwrap();
        let service2 = MusicService::create(
            &pool,
            artist.id,
            Platform::AppleMusic,
            "https://apple.com/artist2".to_string(),
        )
        .await
        .unwrap();

        let services = MusicService::list_by_artist(&pool, artist.id)
            .await
            .unwrap();
        assert_eq!(services.len(), 2);
        assert!(services.contains(&service1));
        assert!(services.contains(&service2));
    }

    #[sqlx::test]
    fn test_update_music_service(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let mut service = MusicService::create(
            &pool,
            artist.id,
            Platform::Spotify,
            "https://spotify.com/artist".to_string(),
        )
        .await
        .unwrap();

        service.url = "https://spotify.com/updated_artist".to_string();
        let updated_service = service.update(&pool).await.unwrap();

        assert_eq!(updated_service.id, service.id);
        assert_eq!(updated_service.artist_id, artist.id);
        assert_eq!(updated_service.platform, Platform::Spotify);
        assert_eq!(
            updated_service.url,
            "https://spotify.com/updated_artist".to_string()
        );
    }

    #[sqlx::test]
    fn test_delete_music_service(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let service = MusicService::create(
            &pool,
            artist.id,
            Platform::Spotify,
            "https://spotify.com/artist".to_string(),
        )
        .await
        .unwrap();

        service.delete(&pool).await.unwrap();

        let services = MusicService::list_by_artist(&pool, artist.id)
            .await
            .unwrap();
        assert!(!services.contains(&service));
    }
}
