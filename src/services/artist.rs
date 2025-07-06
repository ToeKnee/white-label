//! Services for the artist routes
use leptos::prelude::ServerFnError;
use sqlx::PgPool;

use super::authentication_helpers::user_with_permissions;
use crate::forms::artist::{CreateArtistForm, UpdateArtistForm};
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
pub async fn get_artist_service(
    pool: &PgPool,
    slug: String,
) -> Result<ArtistResult, ServerFnError> {
    Ok(ArtistResult {
        artist: Artist::get_by_slug(pool, slug).await.map_err(|e| {
            let err = format!("Error while getting artist: {e:?}");
            tracing::error!("{err}");
            ServerFnError::new(e)
        })?,
    })
}

/// Create a new artist
///
/// # Arguments
/// pool: `PgPool` - The database connection pool
/// user: Option<&User> - The user creating the artist
/// `artist_form`: `CreateArtistForm` - The form to create the artist
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
    pool: &PgPool,
    user: Option<&User>,
    artist_form: CreateArtistForm,
) -> Result<ArtistResult, ServerFnError> {
    match user_with_permissions(user, vec!["admin", "label_owner"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }

    Ok(ArtistResult {
        artist: Artist::create(
            pool,
            artist_form.name,
            artist_form.description,
            artist_form.website,
            artist_form.label_id,
            artist_form.published_at,
        )
        .await
        .map_err(|e| {
            let err = format!("Error while creating artist: {e:?}");
            tracing::error!("{err}");
            ServerFnError::new(e)
        })?,
    })
}

/// Update an artist
///
/// # Arguments
/// pool: `PgPool` - The database connection pool
/// user: Option<&User> - The user updating the artist
/// `artist_form`: `UpdateArtistForm` - The form to update the artist
///
/// # Returns
/// Result<`ArtistResult`, `ServerFnError`> - The updated artist
///
/// # Errors
/// If the name is empty, return an error
/// If the artist cannot be updated, return an error
/// If the user does not have the required permissions, return an error
#[cfg(feature = "ssr")]
pub async fn update_artist_service(
    pool: &PgPool,
    user: Option<&User>,
    artist_form: UpdateArtistForm,
) -> Result<ArtistResult, ServerFnError> {
    match user_with_permissions(user, vec!["admin", "label_owner"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }

    let mut artist = Artist::get_by_slug(pool, artist_form.slug)
        .await
        .map_err(|e| {
            let err = format!("Error while getting artist: {e:?}");
            tracing::error!("{err}");
            ServerFnError::new(e)
        })?;
    artist.name = artist_form.name;
    artist.description = artist_form.description;
    artist.website = artist_form.website;
    artist.published_at = artist_form.published_at;

    Ok(ArtistResult {
        artist: artist.update(pool).await.map_err(|e| {
            let err = format!("Error while updating artist: {e:?}");
            tracing::error!("{err}");
            ServerFnError::new(e)
        })?,
    })
}

/// Soft delete an artist
///
/// # Arguments
/// pool: `PgPool` - The database connection pool
/// user: Option<&User> - The user deleting the artist
/// slug: String - The slug of the artist
///
/// # Returns
/// Result<`ArtistResult`, `ServerFnError`> - The deleted artist
///
/// # Errors
/// If the artist cannot be found, return an error
/// If the user does not have the required permissions, return an error
#[cfg(feature = "ssr")]
pub async fn delete_artist_service(
    pool: &PgPool,
    user: Option<&User>,
    slug: String,
) -> Result<ArtistResult, ServerFnError> {
    match user_with_permissions(user, vec!["admin", "label_owner"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }

    let artist = Artist::get_by_slug(pool, slug).await.map_err(|e| {
        let err = format!("Error while getting artist: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;

    Ok(ArtistResult {
        artist: artist.delete(pool).await.map_err(|e| {
            let err = format!("Error while deleting artist: {e:?}");
            tracing::error!("{err}");
            ServerFnError::new(e)
        })?,
    })
}

/// Restore a soft deleted artist
///
/// # Arguments
/// pool: `PgPool` - The database connection pool
/// user: Option<&User> - The user deleting the artist
/// slug: String - The slug of the artist
///
/// # Returns
/// Result<`ArtistResult`, `ServerFnError`> - The restored artist
///
/// # Errors
/// If the artist cannot be found, return an error
/// If the user does not have the required permissions, return an error
#[cfg(feature = "ssr")]
pub async fn restore_artist_service(
    pool: &PgPool,
    user: Option<&User>,
    slug: String,
) -> Result<ArtistResult, ServerFnError> {
    match user_with_permissions(user, vec!["admin", "label_owner"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }

    let mut artist = Artist::get_by_slug(pool, slug).await.map_err(|e| {
        let err = format!("Error while getting artist: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;
    artist.deleted_at = None;
    artist.clone().update(pool).await.map_err(|e| {
        let err = format!("Error while restoring artist: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;

    Ok(ArtistResult { artist })
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
        let artist_by_slug = get_artist_service(&pool, artist.slug.clone())
            .await
            .unwrap();
        assert_eq!(artist, artist_by_slug.artist);
    }

    #[sqlx::test]
    async fn test_get_artist_service_no_artist(pool: PgPool) {
        let result = get_artist_service(&pool, "missing".to_string()).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "error running server function: Could not find artist with slug missing.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_create_artist_service(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();
        let record_label = create_test_record_label(&pool, 1).await.unwrap();

        let artist_form = CreateArtistForm {
            name: "Test Artist".to_string(),
            description: "This is a test artist".to_string(),
            website: "https://example.com".to_string(),
            label_id: record_label.id,
            published_at: None,
        };

        let artist = create_artist_service(&pool, Some(&user), artist_form)
            .await
            .unwrap();
        assert_eq!(artist.artist.name, "Test Artist".to_string());
        assert_eq!(
            artist.artist.description,
            "This is a test artist".to_string()
        );
    }

    #[sqlx::test]
    async fn test_create_artist_service_no_permission(pool: PgPool) {
        let user = create_test_user_with_permissions(&pool, 1, vec!["admin"]) // but not label_owner
            .await
            .unwrap();
        let record_label = create_test_record_label(&pool, 1).await.unwrap();

        let artist_form = CreateArtistForm {
            name: "Test Artist".to_string(),
            description: "This is a test artist".to_string(),
            website: "https://example.com".to_string(),
            label_id: record_label.id,
            published_at: None,
        };

        let artist = create_artist_service(&pool, Some(&user), artist_form).await;

        assert!(artist.is_err());
        assert_eq!(
            artist.unwrap_err().to_string(),
            "error running server function: You do not have permission.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_create_artist_service_no_name(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();
        let record_label = create_test_record_label(&pool, 1).await.unwrap();

        let artist_form = CreateArtistForm {
            name: String::new(),
            description: "This is a test artist".to_string(),
            website: "https://example.com".to_string(),
            label_id: record_label.id,
            published_at: None,
        };

        let artist = create_artist_service(&pool, Some(&user), artist_form).await;

        assert!(artist.is_err());
        assert_eq!(
            artist.unwrap_err().to_string(),
            "error running server function: Name is required.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_create_artist_service_name_too_long(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();
        let record_label = create_test_record_label(&pool, 1).await.unwrap();

        let name = "a".repeat(256);
        let artist_form = CreateArtistForm {
            name,
            description: "This is a test artist".to_string(),
            website: "https://example.com".to_string(),
            label_id: record_label.id,
            published_at: None,
        };
        let artist = create_artist_service(&pool, Some(&user), artist_form).await;

        assert!(artist.is_err());
        assert_eq!(
            artist.unwrap_err().to_string(),
            "error running server function: Name must be less than 255 characters.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_create_artist_service_no_record_label(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let artist_form = CreateArtistForm {
            name: "Test Artist".to_string(),
            description: "This is a test artist".to_string(),
            website: "https://example.com".to_string(),
            label_id: 0,
            published_at: None,
        };

        let artist = create_artist_service(&pool, Some(&user), artist_form).await;

        assert!(artist.is_err());
        assert_eq!(
            artist.unwrap_err().to_string(),
            "error running server function: Record Label with id 0 does not exist.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_update_artist_service(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let artist_form = UpdateArtistForm {
            slug: artist.slug.clone(),
            name: "Updated Artist".to_string(),
            description: "This is an updated artist".to_string(),
            website: "https://update.example.com".to_string(),
            published_at: Some(chrono::Utc::now()),
        };
        let updated_artist = update_artist_service(&pool, Some(&user), artist_form)
            .await
            .unwrap();
        assert_eq!(updated_artist.artist.name, "Updated Artist".to_string());
        assert_eq!(
            updated_artist.artist.description,
            "This is an updated artist".to_string()
        );
        assert_eq!(
            updated_artist.artist.website,
            "https://update.example.com".to_string()
        );
    }

    #[sqlx::test]
    async fn test_update_artist_service_name_is_empty(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let artist_form = UpdateArtistForm {
            slug: artist.slug.clone(),
            name: String::new(),
            description: "This is an updated artist".to_string(),
            website: "https://example.com".to_string(),
            published_at: Some(chrono::Utc::now()),
        };
        let updated_artist = update_artist_service(&pool, Some(&user), artist_form).await;

        assert!(updated_artist.is_err());
        assert_eq!(
            updated_artist.unwrap_err().to_string(),
            "error running server function: Name is required.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_update_artist_service_name_too_long(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let name = "a".repeat(256);
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let artist_form = UpdateArtistForm {
            slug: artist.slug.clone(),
            name,
            description: "This is an updated artist".to_string(),
            website: "https://example.com".to_string(),
            published_at: Some(chrono::Utc::now()),
        };
        let updated_artist = update_artist_service(&pool, Some(&user), artist_form).await;

        assert!(updated_artist.is_err());
        assert_eq!(
            updated_artist.unwrap_err().to_string(),
            "error running server function: Name must be less than 255 characters.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_update_artist_service_no_artist(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let artist_form = UpdateArtistForm {
            slug: "missing".to_string(),
            name: "Updated Artist".to_string(),
            description: "This is an updated artist".to_string(),
            website: "https://example.com".to_string(),
            published_at: Some(chrono::Utc::now()),
        };
        let updated_artist = update_artist_service(&pool, Some(&user), artist_form).await;

        assert!(updated_artist.is_err());
        assert_eq!(
            updated_artist.unwrap_err().to_string(),
            "error running server function: Could not find artist with slug missing.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_update_artist_service_no_user(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let artist_form = UpdateArtistForm {
            slug: artist.slug.clone(),
            name: "Updated Artist".to_string(),
            description: "This is an updated artist".to_string(),
            website: "https://example.com".to_string(),
            published_at: Some(chrono::Utc::now()),
        };
        let updated_artist =
            update_artist_service(&pool, Some(&User::default()), artist_form).await;

        assert!(updated_artist.is_err());
        assert_eq!(
            updated_artist.unwrap_err().to_string(),
            "error running server function: You must be logged in to view this page.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_update_artist_service_no_permissions(pool: PgPool) {
        let user = create_test_user_with_permissions(&pool, 1, vec![])
            .await
            .unwrap();
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let artist_form = UpdateArtistForm {
            slug: artist.slug.clone(),
            name: "Updated Artist".to_string(),
            description: "This is an updated artist".to_string(),
            website: "https://example.com".to_string(),
            published_at: Some(chrono::Utc::now()),
        };
        let updated_artist = update_artist_service(&pool, Some(&user), artist_form).await;

        assert!(updated_artist.is_err());
        assert_eq!(
            updated_artist.unwrap_err().to_string(),
            "error running server function: You do not have permission.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_delete_artist_service(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let deleted_artist = delete_artist_service(&pool, Some(&user), artist.slug.clone())
            .await
            .unwrap();
        assert!(deleted_artist.artist.deleted_at.is_some());
    }

    #[sqlx::test]
    async fn test_delete_artist_service_no_artist(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let deleted_artist = delete_artist_service(&pool, Some(&user), "missing".to_string()).await;
        assert!(deleted_artist.is_err());
        assert_eq!(
            deleted_artist.unwrap_err().to_string(),
            "error running server function: Could not find artist with slug missing.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_delete_artist_service_no_user(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let deleted_artist =
            delete_artist_service(&pool, Some(&User::default()), artist.slug.clone()).await;
        assert!(deleted_artist.is_err());
        assert_eq!(
            deleted_artist.unwrap_err().to_string(),
            "error running server function: You must be logged in to view this page.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_delete_artist_service_no_permissions(pool: PgPool) {
        let user = create_test_user_with_permissions(&pool, 1, vec![])
            .await
            .unwrap();
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let deleted_artist = delete_artist_service(&pool, Some(&user), artist.slug.clone()).await;
        assert!(deleted_artist.is_err());
        assert_eq!(
            deleted_artist.unwrap_err().to_string(),
            "error running server function: You do not have permission.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_restore_artist_service(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let artist = artist.delete(&pool).await.unwrap();
        let restored_artist = restore_artist_service(&pool, Some(&user), artist.slug.clone())
            .await
            .unwrap();

        assert!(restored_artist.artist.deleted_at.is_none());
    }

    #[sqlx::test]
    async fn test_restore_artist_service_no_artist(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let restored_artist =
            restore_artist_service(&pool, Some(&user), "missing".to_string()).await;
        assert!(restored_artist.is_err());
        assert_eq!(
            restored_artist.unwrap_err().to_string(),
            "error running server function: Could not find artist with slug missing.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_restore_artist_service_no_user(pool: PgPool) {
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let artist = artist.delete(&pool).await.unwrap();
        let restored_artist =
            restore_artist_service(&pool, Some(&User::default()), artist.slug.clone()).await;
        assert!(restored_artist.is_err());
        assert_eq!(
            restored_artist.unwrap_err().to_string(),
            "error running server function: You must be logged in to view this page.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_restore_artist_service_no_permissions(pool: PgPool) {
        let user = create_test_user_with_permissions(&pool, 1, vec![])
            .await
            .unwrap();
        let artist = create_test_artist(&pool, 1, None).await.unwrap();
        let artist = artist.delete(&pool).await.unwrap();
        let restored_artist = restore_artist_service(&pool, Some(&user), artist.slug.clone()).await;
        assert!(restored_artist.is_err());
        assert_eq!(
            restored_artist.unwrap_err().to_string(),
            "error running server function: You do not have permission.".to_string()
        );
    }
}
