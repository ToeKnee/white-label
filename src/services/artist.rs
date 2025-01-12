//! Services for the artist routes
use leptos::prelude::ServerFnError;
use sqlx::PgPool;

use super::authentication_helpers::user_with_permissions;
use crate::models::{artist::Artist, auth::User};
use crate::routes::artist::ArtistResult;

/// Get an artist by slug
///
/// # Arguments
/// pool: `PgPool` - The database connection pool
/// slug: String - The slug of the artist
///
/// # Returns
/// Result<`ArtistResult`, `ServerFnError`> - The artist
///
/// # Errors
/// If the artist cannot be found, return an error
pub async fn get_artist_service(pool: PgPool, slug: String) -> Result<ArtistResult, ServerFnError> {
    Ok(ArtistResult {
        artist: Artist::get_by_slug(&pool, slug).await.map_err(|x| {
            let err = format!("Error while getting artist: {x:?}");
            tracing::error!("{err}");
            ServerFnError::new("Could not retrieve artist, try again later")
        })?,
    })
}

/// Create a new artist
///
/// # Arguments
/// pool: `PgPool` - The database connection pool
/// user: Option<&User> - The user creating the artist
/// name: String - The name of the artist
/// description: String - The description of the artist
/// `record_label_id`: i64 - The ID of the record label the artist is signed to
///
/// # Returns
/// Result<`ArtistResult`, `ServerFnError`> - The created artist
///
/// # Errors
/// If the name is empty, return an error
/// If the artist cannot be created, return an error
/// If the user does not have the required permissions, return an error
#[cfg(feature = "ssr")]
pub async fn create_artist_service(
    pool: PgPool,
    user: Option<&User>,
    name: String,
    description: String,
    record_label_id: i64,
) -> Result<ArtistResult, ServerFnError> {
    let _user = user_with_permissions(user, vec!["admin", "label_owner"]);

    if name.is_empty() {
        return Err(ServerFnError::new("Name cannot be empty."));
    }

    Ok(ArtistResult {
        artist: Artist::create(&pool, name, description, record_label_id)
            .await
            .map_err(|x| {
                let err = format!("Error while creating artist: {x:?}");
                tracing::error!("{err}");
                ServerFnError::new("Could not create artist, try again later")
            })?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "ssr")]
    use crate::models::test_helpers::{
        create_test_artist, create_test_record_label, create_test_user_with_permissions,
    };

    #[sqlx::test]
    async fn test_get_artist_service(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let artist_by_slug = get_artist_service(pool, artist.slug.clone()).await.unwrap();
        assert_eq!(artist, artist_by_slug.artist);
    }

    #[sqlx::test]
    async fn test_get_artist_service_no_artist(pool: PgPool) {
        let result = get_artist_service(pool, "missing".to_string()).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "error running server function: Could not retrieve artist, try again later".to_string()
        );
    }

    #[sqlx::test]
    async fn test_create_artist_service(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();
        let record_label = create_test_record_label(&pool, 1).await.unwrap();

        let artist = create_artist_service(
            pool,
            Some(&user),
            "Test Artist".to_string(),
            "This is a test artist".to_string(),
            record_label.id,
        )
        .await
        .unwrap();
        assert_eq!(artist.artist.name, "Test Artist".to_string());
        assert_eq!(
            artist.artist.description,
            "This is a test artist".to_string()
        );
    }

    #[sqlx::test]
    async fn test_create_artist_service_no_name(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();
        let record_label = create_test_record_label(&pool, 1).await.unwrap();

        let artist = create_artist_service(
            pool,
            Some(&user),
            String::new(),
            "This is a test artist".to_string(),
            record_label.id,
        )
        .await;

        assert!(artist.is_err());
        assert_eq!(
            artist.unwrap_err().to_string(),
            "error running server function: Name cannot be empty.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_create_artist_service_no_record_label(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let artist = create_artist_service(
            pool,
            Some(&user),
            "Test Artist".to_string(),
            "This is a test artist".to_string(),
            0,
        )
        .await;

        assert!(artist.is_err());
        assert_eq!(
            artist.unwrap_err().to_string(),
            "error running server function: Could not create artist, try again later".to_string()
        );
    }
}
