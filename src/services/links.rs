//! Module for routes handling music services and social links associated with artists.
use leptos::prelude::ServerFnError;
use sqlx::PgPool;

use crate::models::{
    artist::Artist, music_service::MusicService, social_media::SocialMediaService,
};
use crate::routes::links::LinksResult;

/// Get all music services for a specific artist.
///
/// # Arguments
/// * `artist_slug`: The slug of the artist.
///
/// # Returns
/// * A `MusicServicesResult` containing a vector of music services associated with the specified artist.
///
/// # Errors
/// Will return a `ServerFnError` if the artist cannot be found, or if there is an issue with the database connection.
pub async fn get_links_service(
    pool: &PgPool,
    artist_slug: String,
) -> Result<LinksResult, ServerFnError> {
    let artist = match Artist::get_by_slug(pool, artist_slug).await {
        Ok(artist) => artist,
        Err(e) => return Err(ServerFnError::new(format!("Artist not found: {e}"))),
    };

    let music_services = MusicService::list_by_artist(pool, artist.id);
    let social_media_services = SocialMediaService::list_by_artist(pool, artist.id);

    match (music_services.await, social_media_services.await) {
        (Ok(services), Ok(social_media_services)) => Ok(LinksResult {
            music_services: services,
            social_media_services,
        }),
        (Err(e), _) | (_, Err(e)) => Err(ServerFnError::new(format!("Error fetching links: {e}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "ssr")]
    use crate::models::{
        music_service::Platform, social_media::SocialMedia, test_helpers::create_test_artist,
    };

    #[sqlx::test]
    async fn test_get_music_services(pool: PgPool) {
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

        let result = get_links_service(&pool, artist.slug).await;
        assert!(result.is_ok());
        if let Ok(LinksResult {
            music_services,
            social_media_services,
        }) = result
        {
            assert_eq!(music_services.len(), 2);
            assert!(music_services.contains(&service1));
            assert!(music_services.contains(&service2));
            assert!(social_media_services.is_empty());
        }
    }

    #[sqlx::test]
    async fn test_get_links_with_social_links(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let service1 = SocialMediaService::create(
            &pool,
            artist.id,
            SocialMedia::Twitter,
            "https://twitter.com/artist1".to_string(),
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

        let result = get_links_service(&pool, artist.slug).await;
        assert!(result.is_ok());
        if let Ok(LinksResult {
            music_services,
            social_media_services,
        }) = result
        {
            assert!(music_services.is_empty());
            assert_eq!(social_media_services.len(), 2);
            assert!(social_media_services.contains(&service1));
            assert!(social_media_services.contains(&service2));
        }
    }

    #[sqlx::test]
    async fn test_get_links_no_artist(pool: PgPool) {
        let result = get_links_service(&pool, "nonexistent-artist".to_string()).await;
        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(
                e.to_string(),
                "error running server function: Artist not found: Could not find artist with slug nonexistent-artist."
            );
        }
    }
}
