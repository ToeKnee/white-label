//! Module for routes handling music services and social links associated with artists.
use leptos::prelude::ServerFnError;
use sqlx::PgPool;

use super::authentication_helpers::user_with_permissions;
use crate::forms::links::LinksForm;
use crate::models::{
    artist::Artist,
    auth::User,
    music_service::{MusicService, Platform},
    social_media::{SocialMedia, SocialMediaService},
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

/// Update music services and social media links for an artist.
///
/// # Arguments
/// * `pool`: The database connection pool.
/// * `user`: The user performing the update.
/// * `form`: The form containing the music service and social links to update.
///
/// # Returns
/// * A `LinksResult` containing the updated music services and social media links.
///
/// # Errors
/// Will return a `ServerFnError` if the artist cannot be found, or if there
/// is an issue with the database connection.
pub async fn update_links_service(
    pool: &PgPool,
    user: Option<&User>,
    form: LinksForm,
) -> Result<LinksResult, ServerFnError> {
    tracing::warn!("DO LOADS OF TESTING!");
    match user_with_permissions(user, vec!["admin", "label_owner"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }

    let artist = match Artist::get_by_slug(pool, form.artist_slug.clone()).await {
        Ok(artist) => artist,
        Err(e) => return Err(ServerFnError::new(format!("Artist not found: {e}"))),
    };

    tracing::info!("Updating links for artist: {}", artist.slug);

    handle_music_services(pool, form.clone(), artist.clone())
        .await
        .map_err(|e| ServerFnError::new(format!("Error handling music services: {e}")))?;

    handle_social_media_services(pool, form, artist.clone())
        .await
        .map_err(|e| ServerFnError::new(format!("Error handling social media services: {e}")))?;

    Ok(LinksResult {
        music_services: MusicService::list_by_artist(pool, artist.id)
            .await
            .map_err(|e| {
                ServerFnError::new(format!("Error fetching updated music services: {e}"))
            })?,
        social_media_services: SocialMediaService::list_by_artist(pool, artist.id)
            .await
            .map_err(|e| {
                ServerFnError::new(format!("Error fetching updated social media services: {e}"))
            })?,
    })
}

async fn handle_music_services(
    pool: &PgPool,
    form: LinksForm,
    artist: Artist,
) -> Result<(), ServerFnError> {
    // Get existing music services and social media links
    let existing_music_services = match MusicService::list_by_artist(pool, artist.id).await {
        Ok(services) => services.clone(),
        Err(e) => {
            return Err(ServerFnError::new(format!(
                "Error fetching music services: {e}"
            )));
        }
    };
    tracing::info!("existing music services: {:?}", existing_music_services);

    let (music_services_to_create, music_services_to_update, music_services_to_delete) =
        categorise_music_services(existing_music_services.clone(), &form);

    create_music_services(pool, music_services_to_create, form.clone(), artist.clone())
        .await
        .map_err(|e| ServerFnError::new(format!("Error creating music services: {e}")))?;

    update_music_services(
        pool,
        music_services_to_update,
        existing_music_services.clone(),
        form.clone(),
        artist.clone(),
    )
    .await
    .map_err(|e| ServerFnError::new(format!("Error updating music services: {e}")))?;

    delete_music_services(
        pool,
        music_services_to_delete,
        existing_music_services.clone(),
        artist.clone(),
    )
    .await
    .map_err(|e| ServerFnError::new(format!("Error deleting music services: {e}")))?;

    Ok(())
}

fn categorise_music_services(
    mut existing_music_services: Vec<MusicService>,
    form: &LinksForm,
) -> (Vec<&Platform>, Vec<&Platform>, Vec<&Platform>) {
    let mut music_services_to_create = vec![];
    let mut music_services_to_update = vec![];
    let mut music_services_to_delete = vec![];

    for platform in Platform::iterator() {
        let url = form.clone().from_platform(platform);
        if let Some(service) = existing_music_services
            .iter_mut()
            .find(|s| s.platform == *platform)
        {
            if url.is_empty() {
                music_services_to_delete.push(platform);
                continue;
            }
            if service.url == url {
                // No change needed
                tracing::debug!(
                    "No change for platform: {:?}, url: {}",
                    platform,
                    service.url
                );
                continue;
            }
            music_services_to_update.push(platform);
        } else {
            // Create new service
            if !url.is_empty() {
                music_services_to_create.push(platform);
            }
        }
    }

    tracing::info!(
        "create_musical_services: {:?}, update_musical_services: {:?}, delete_musical_services: {:?}",
        music_services_to_create,
        music_services_to_update,
        music_services_to_delete
    );
    (
        music_services_to_create,
        music_services_to_update,
        music_services_to_delete,
    )
}

async fn create_music_services(
    pool: &PgPool,
    music_services: Vec<&Platform>,
    form: LinksForm,
    artist: Artist,
) -> Result<(), ServerFnError> {
    for platform in music_services {
        MusicService::create(
            pool,
            artist.id,
            platform.clone(),
            form.clone().from_platform(platform),
        )
        .await
        .map_err(|e| ServerFnError::new(format!("Error creating music service: {e}")))?;
    }
    Ok(())
}

async fn update_music_services(
    pool: &PgPool,
    music_services: Vec<&Platform>,
    mut existing_music_services: Vec<MusicService>,
    form: LinksForm,
    artist: Artist,
) -> Result<(), ServerFnError> {
    for platform in music_services {
        if let Some(service) = existing_music_services
            .iter_mut()
            .find(|s| s.platform == *platform)
        {
            let url = form.clone().from_platform(platform);
            match sqlx::query(
                "UPDATE music_services SET url = $1
                WHERE artist_id = $2
                AND platform = $3
                ",
            )
            .bind(url.clone())
            .bind(artist.id)
            .bind(service.platform.clone())
            .execute(pool)
            .await
            {
                Ok(artist) => artist,
                Err(e) => {
                    tracing::error!("{e}");
                    return Err(ServerFnError::new(format!(
                        "Could not update music_services with url {}, artist_id {}, platform {}.",
                        url, artist.id, platform,
                    )));
                }
            };
        }
    }
    Ok(())
}

async fn delete_music_services(
    pool: &PgPool,
    music_services: Vec<&Platform>,
    mut existing_music_services: Vec<MusicService>,
    artist: Artist,
) -> Result<(), ServerFnError> {
    for platform in music_services {
        if let Some(index) = existing_music_services
            .iter()
            .position(|s| s.platform == *platform)
        {
            existing_music_services.remove(index);
            sqlx::query("DELETE FROM music_services WHERE artist_id = $1 AND platform = $2")
                .bind(artist.id)
                .bind(platform)
                .execute(pool)
                .await
                .map_err(|e| ServerFnError::new(format!("Error deleting music service: {e}")))?;
        }
    }
    Ok(())
}

async fn handle_social_media_services(
    pool: &PgPool,
    form: LinksForm,
    artist: Artist,
) -> Result<(), ServerFnError> {
    let existing_social_media_services =
        match SocialMediaService::list_by_artist(pool, artist.id).await {
            Ok(services) => services,
            Err(e) => {
                return Err(ServerFnError::new(format!(
                    "Error fetching social media services: {e}"
                )));
            }
        };
    tracing::info!(
        "existing social media services: {:?}",
        existing_social_media_services
    );

    let (social_media_to_create, social_media_to_update, social_media_to_delete) =
        categorise_social_media_services(existing_social_media_services.clone(), &form);

    create_social_media_services(pool, social_media_to_create, form.clone(), artist.clone())
        .await
        .map_err(|e| ServerFnError::new(format!("Error creating social media services: {e}")))?;

    update_social_media_services(
        pool,
        social_media_to_update,
        existing_social_media_services.clone(),
        form.clone(),
        artist.clone(),
    )
    .await
    .map_err(|e| ServerFnError::new(format!("Error updating social media services: {e}")))?;

    delete_social_media_services(
        pool,
        social_media_to_delete,
        existing_social_media_services.clone(),
        artist.clone(),
    )
    .await
    .map_err(|e| ServerFnError::new(format!("Error deleting social media services: {e}")))?;
    Ok(())
}

fn categorise_social_media_services(
    mut existing_social_media_services: Vec<SocialMediaService>,
    form: &LinksForm,
) -> (Vec<SocialMedia>, Vec<SocialMedia>, Vec<SocialMedia>) {
    let mut social_media_to_create = vec![];
    let mut social_media_to_update = vec![];
    let mut social_media_to_delete = vec![];

    for platform in SocialMedia::iterator() {
        let url = form.clone().from_social_media(platform);
        if let Some(service) = existing_social_media_services
            .iter_mut()
            .find(|s| s.platform == *platform)
        {
            if url.is_empty() {
                social_media_to_delete.push(platform.clone());
            } else if url != service.url {
                social_media_to_update.push(platform.clone());
            }
        } else {
            // Create new service
            if !url.is_empty() {
                social_media_to_create.push(platform.clone());
            }
        }
    }

    tracing::info!(
        "create_social_media_services: {:?}, update_social_media_services: {:?}, delete_social_media_services: {:?}",
        social_media_to_create,
        social_media_to_update,
        social_media_to_delete
    );
    (
        social_media_to_create,
        social_media_to_update,
        social_media_to_delete,
    )
}

async fn create_social_media_services(
    pool: &PgPool,
    social_media_services: Vec<SocialMedia>,
    form: LinksForm,
    artist: Artist,
) -> Result<(), ServerFnError> {
    for platform in social_media_services {
        SocialMediaService::create(
            pool,
            artist.id,
            platform.clone(),
            form.clone().from_social_media(&platform),
        )
        .await
        .map_err(|e| ServerFnError::new(format!("Error creating social media service: {e}")))?;
    }
    Ok(())
}

async fn update_social_media_services(
    pool: &PgPool,
    social_media_services: Vec<SocialMedia>,
    mut existing_social_media_services: Vec<SocialMediaService>,
    form: LinksForm,
    artist: Artist,
) -> Result<(), ServerFnError> {
    for platform in social_media_services {
        if let Some(service) = existing_social_media_services
            .iter_mut()
            .find(|s| s.platform == platform)
        {
            let url = form.clone().from_social_media(&platform);
            match sqlx::query(
                "UPDATE social_media SET url = $1
                WHERE artist_id = $2
                AND platform = $3",
            )
            .bind(url.clone())
            .bind(artist.id)
            .bind(service.platform.clone())
            .execute(pool)
            .await
            {
                Ok(_) => (),
                Err(e) => {
                    tracing::error!("{e}");
                    return Err(ServerFnError::new(format!(
                        "Could not update social media service with url {}, artist_id {}, platform {}.",
                        url, artist.id, platform,
                    )));
                }
            }
        }
    }
    Ok(())
}

async fn delete_social_media_services(
    pool: &PgPool,
    social_media_services: Vec<SocialMedia>,
    mut existing_social_media_services: Vec<SocialMediaService>,
    artist: Artist,
) -> Result<(), ServerFnError> {
    for platform in social_media_services {
        if let Some(index) = existing_social_media_services
            .iter()
            .position(|s| s.platform == platform)
        {
            existing_social_media_services.remove(index);
            sqlx::query("DELETE FROM social_media WHERE artist_id = $1 AND platform = $2")
                .bind(artist.id)
                .bind(&platform)
                .execute(pool)
                .await
                .map_err(|e| {
                    tracing::error!("{e}");
                    ServerFnError::new(format!("Error deleting social media service: {e}"))
                })?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "ssr")]
    use crate::models::{
        music_service::Platform,
        social_media::SocialMedia,
        test_helpers::{create_test_artist, create_test_user_with_permissions},
    };

    #[sqlx::test]
    async fn test_get_links_with_music_services(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let service1 = MusicService::create(
            &pool,
            artist.id,
            Platform::Spotify,
            "https://spotify.com/artist".to_string(),
        )
        .await
        .unwrap();
        let service2 = MusicService::create(
            &pool,
            artist.id,
            Platform::AppleMusic,
            "https://apple.com/artist".to_string(),
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
            "https://twitter.com/artist".to_string(),
        )
        .await
        .unwrap();
        let service2 = SocialMediaService::create(
            &pool,
            artist.id,
            SocialMedia::Instagram,
            "https://instagram.com/artist".to_string(),
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

    #[sqlx::test]
    async fn test_update_links_not_admin(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let user = create_test_user_with_permissions(&pool, 1, vec![])
            .await
            .unwrap();
        let form = LinksForm {
            artist_slug: artist.slug.clone(),
            spotify: "https://spotify.com/artist".to_string(),
            apple_music: "https://apple.com/artist".to_string(),
            ..Default::default()
        };
        let result = update_links_service(&pool, Some(&user), form).await;
        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(
                e.to_string(),
                "error running server function: You do not have permission."
            );
        }
    }

    #[sqlx::test]
    async fn test_update_links_with_social_media(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let user = create_test_user_with_permissions(&pool, 1, vec!["admin", "label_owner"])
            .await
            .unwrap();

        let form = LinksForm {
            artist_slug: artist.slug.clone(),
            twitter: "https://twitter.com/artist".to_string(),
            instagram: "https://instagram.com/artist".to_string(),
            ..Default::default()
        };
        let result = update_links_service(&pool, Some(&user), form).await;
        assert!(result.is_ok());
        if let Ok(LinksResult {
            music_services,
            social_media_services,
        }) = result
        {
            assert!(music_services.is_empty());
            assert_eq!(social_media_services.len(), 2);
            assert!(
                social_media_services
                    .iter()
                    .any(|s| s.platform == SocialMedia::Twitter)
            );
            assert!(
                social_media_services
                    .iter()
                    .any(|s| s.platform == SocialMedia::Instagram)
            );
        }
    }

    #[sqlx::test]
    async fn test_update_links_with_music_services(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let user = create_test_user_with_permissions(&pool, 1, vec!["admin", "label_owner"])
            .await
            .unwrap();

        let form = LinksForm {
            artist_slug: artist.slug.clone(),
            spotify: "https://spotify.com/artist".to_string(),
            apple_music: "https://apple.com/artist".to_string(),
            ..Default::default()
        };
        let result = update_links_service(&pool, Some(&user), form).await;
        assert!(result.is_ok());
        if let Ok(LinksResult {
            music_services,
            social_media_services,
        }) = result
        {
            assert_eq!(music_services.len(), 2);
            assert!(
                music_services
                    .iter()
                    .any(|s| s.platform == Platform::Spotify)
            );
            assert!(
                music_services
                    .iter()
                    .any(|s| s.platform == Platform::AppleMusic)
            );
            assert!(social_media_services.is_empty());
        }
    }

    #[sqlx::test]
    async fn test_handle_music_services(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let form = LinksForm {
            artist_slug: artist.slug.clone(),
            spotify: "https://spotify.com/artist".to_string(),
            apple_music: "https://apple.com/artist".to_string(),
            ..Default::default()
        };

        let result = handle_music_services(&pool, form, artist.clone()).await;
        assert!(result.is_ok());

        let music_services = MusicService::list_by_artist(&pool, artist.id)
            .await
            .unwrap();
        assert_eq!(music_services.len(), 2);
    }

    #[sqlx::test]
    async fn test_handle_social_media_services(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let form = LinksForm {
            artist_slug: artist.slug.clone(),
            twitter: "https://twitter.com/artist".to_string(),
            instagram: "https://instagram.com/artist".to_string(),
            ..Default::default()
        };
        let result = handle_social_media_services(&pool, form, artist.clone()).await;
        assert!(result.is_ok());
        let social_media_services = SocialMediaService::list_by_artist(&pool, artist.id)
            .await
            .unwrap();
        assert_eq!(social_media_services.len(), 2);
        assert!(
            social_media_services
                .iter()
                .any(|s| s.platform == SocialMedia::Twitter)
        );
        assert!(
            social_media_services
                .iter()
                .any(|s| s.platform == SocialMedia::Instagram)
        );
    }

    #[sqlx::test]
    async fn test_categorise_music_services(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let existing_services = vec![
            MusicService::create(
                &pool,
                artist.id,
                Platform::Spotify,
                "https://spotify.com/artist".to_string(),
            )
            .await
            .unwrap(),
            MusicService::create(
                &pool,
                artist.id,
                Platform::AppleMusic,
                "https://apple.com/artist".to_string(),
            )
            .await
            .unwrap(),
            MusicService::create(
                &pool,
                artist.id,
                Platform::YouTubeMusic,
                "https://youtube.com/artist".to_string(),
            )
            .await
            .unwrap(),
        ];
        let form = LinksForm {
            artist_slug: artist.slug.clone(),
            spotify: "https://spotify.com/artist/changed".to_string(),
            apple_music: "https://apple.com/artist".to_string(),
            tidal: "https://tidal.com/artist".to_string(),
            ..Default::default()
        };
        let (to_create, to_update, to_delete) = categorise_music_services(existing_services, &form);
        assert_eq!(to_create.len(), 1);
        assert_eq!(to_update.len(), 1);
        assert_eq!(to_delete.len(), 1);
        assert!(to_create.contains(&&Platform::Tidal));
        assert!(to_update.contains(&&Platform::Spotify));
        assert!(to_delete.contains(&&Platform::YouTubeMusic));
    }

    #[sqlx::test]
    async fn test_categorise_social_media_services(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let existing_services = vec![
            SocialMediaService::create(
                &pool,
                artist.id,
                SocialMedia::Twitter,
                "https://twitter.com/artist".to_string(),
            )
            .await
            .unwrap(),
            SocialMediaService::create(
                &pool,
                artist.id,
                SocialMedia::Instagram,
                "https://instagram.com/artist".to_string(),
            )
            .await
            .unwrap(),
            SocialMediaService::create(
                &pool,
                artist.id,
                SocialMedia::Facebook,
                "https://facebook.com/artist".to_string(),
            )
            .await
            .unwrap(),
        ];
        let form = LinksForm {
            artist_slug: artist.slug.clone(),
            twitter: "https://twitter.com/artist/changed".to_string(),
            instagram: "https://instagram.com/artist".to_string(),
            you_tube: "https://youtube.com/artist".to_string(),
            ..Default::default()
        };
        let (to_create, to_update, to_delete) =
            categorise_social_media_services(existing_services, &form);
        assert_eq!(to_create.len(), 1);
        assert_eq!(to_update.len(), 1);
        assert_eq!(to_delete.len(), 1);
        assert!(to_create.contains(&SocialMedia::YouTube));
        assert!(to_update.contains(&SocialMedia::Twitter));
        assert!(to_delete.contains(&SocialMedia::Facebook));
    }

    #[sqlx::test]
    async fn test_create_music_services(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let form = LinksForm {
            artist_slug: artist.slug.clone(),
            spotify: "https://spotify.com/artist".to_string(),
            ..Default::default()
        };
        let result =
            create_music_services(&pool, vec![&Platform::Spotify], form, artist.clone()).await;
        assert!(result.is_ok());
        let music_services = MusicService::list_by_artist(&pool, artist.id)
            .await
            .unwrap();
        assert_eq!(music_services.len(), 1);
        assert_eq!(music_services[0].platform, Platform::Spotify);
    }

    #[sqlx::test]
    async fn test_create_social_media_services(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let form = LinksForm {
            artist_slug: artist.slug.clone(),
            twitter: "https://twitter.com/artist".to_string(),
            ..Default::default()
        };
        let result =
            create_social_media_services(&pool, vec![SocialMedia::Twitter], form, artist.clone())
                .await;
        assert!(result.is_ok());
        let social_media_services = SocialMediaService::list_by_artist(&pool, artist.id)
            .await
            .unwrap();
        assert_eq!(social_media_services.len(), 1);
        assert_eq!(social_media_services[0].platform, SocialMedia::Twitter);
    }

    #[sqlx::test]
    async fn test_update_music_services(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let existing_service = MusicService::create(
            &pool,
            artist.id,
            Platform::Spotify,
            "https://spotify.com/artist".to_string(),
        )
        .await
        .unwrap();
        let form = LinksForm {
            artist_slug: artist.slug.clone(),
            spotify: "https://spotify.com/artist/updated".to_string(),
            ..Default::default()
        };
        let result = update_music_services(
            &pool,
            vec![&Platform::Spotify],
            vec![existing_service],
            form,
            artist.clone(),
        )
        .await;
        assert!(result.is_ok());
        let music_services = MusicService::list_by_artist(&pool, artist.id)
            .await
            .unwrap();
        assert_eq!(music_services.len(), 1);
        assert_eq!(music_services[0].platform, Platform::Spotify);
        assert_eq!(music_services[0].url, "https://spotify.com/artist/updated");
    }

    #[sqlx::test]
    async fn test_update_social_media_services(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let existing_service = SocialMediaService::create(
            &pool,
            artist.id,
            SocialMedia::Twitter,
            "https://twitter.com/artist".to_string(),
        )
        .await
        .unwrap();
        let form = LinksForm {
            artist_slug: artist.slug.clone(),
            twitter: "https://twitter.com/artist/updated".to_string(),
            ..Default::default()
        };
        let result = update_social_media_services(
            &pool,
            vec![SocialMedia::Twitter],
            vec![existing_service],
            form,
            artist.clone(),
        )
        .await;
        assert!(result.is_ok());
        let social_media_services = SocialMediaService::list_by_artist(&pool, artist.id)
            .await
            .unwrap();
        assert_eq!(social_media_services.len(), 1);
        assert_eq!(social_media_services[0].platform, SocialMedia::Twitter);
        assert_eq!(
            social_media_services[0].url,
            "https://twitter.com/artist/updated"
        );
    }

    #[sqlx::test]
    async fn test_delete_music_services(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let existing_service = MusicService::create(
            &pool,
            artist.id,
            Platform::Spotify,
            "https://spotify.com/artist".to_string(),
        )
        .await
        .unwrap();
        let result = delete_music_services(
            &pool,
            vec![&Platform::Spotify],
            vec![existing_service],
            artist.clone(),
        )
        .await;
        assert!(result.is_ok());
        let music_services = MusicService::list_by_artist(&pool, artist.id)
            .await
            .unwrap();
        assert!(music_services.is_empty());
    }

    #[sqlx::test]
    async fn test_delete_social_media_services(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let existing_service = SocialMediaService::create(
            &pool,
            artist.id,
            SocialMedia::Twitter,
            "https://twitter.com/artist".to_string(),
        )
        .await
        .unwrap();
        let result = delete_social_media_services(
            &pool,
            vec![SocialMedia::Twitter],
            vec![existing_service],
            artist.clone(),
        )
        .await;
        assert!(result.is_ok());
        let social_media_services = SocialMediaService::list_by_artist(&pool, artist.id)
            .await
            .unwrap();
        assert!(social_media_services.is_empty());
    }
}
