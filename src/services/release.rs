//! Services for the releases routes
use leptos::prelude::ServerFnError;
use sqlx::PgPool;

use super::authentication_helpers::user_with_permissions;
use crate::forms::release::{CreateReleaseForm, UpdateReleaseForm};
use crate::models::{artist::Artist, auth::User, release::Release};
use crate::routes::release::{ReleaseResult, ReleasesResult};

/// Get an artists releases
///
/// # Arguments
/// * `pool` - The database connection pool
/// * `user` - The current user
/// * `slug` - The slug of the artist
///
/// # Returns
/// The releases for the artist
///
/// # Errors
/// If the artist cannot be found, return an error
/// If the user cannot be found, return an error
/// If the releases cannot be found, return an error
#[cfg(feature = "ssr")]
pub async fn get_releases_service(pool: &PgPool, user: Option<&User>, slug: String) -> Result<ReleasesResult, ServerFnError> {
    let artist = match Artist::get_by_slug(pool, slug).await {
        Ok(artist) => artist,
        Err(e) => {
            let err = format!("Error while getting artist: {e:?}");
            tracing::error!("{err}");
            return Err(ServerFnError::new(e));
        }
    };

    let include_hidden = user.is_some_and(|current_user| current_user.permissions.contains("label_owner"));

    Ok(ReleasesResult {
        releases: match Release::list_by_artist_and_record_label(pool, artist.id, artist.label_id, include_hidden).await {
            Ok(releases) => releases,
            Err(e) => {
                let err = format!("Error while getting releases: {e:?}");
                tracing::error!("{err}");
                return Err(ServerFnError::new(e));
            }
        },
    })
}

/// Get a specific artists release
///
/// # Arguments
/// * `pool` - The database connection pool
/// * `user` - The current user
/// * `artist_slug` - The slug of the artist
/// * `release_slug` - The slug of the release
///
/// # Returns
/// The release for the artist
///
/// # Errors
/// If the artist cannot be found, return an error
/// If the user cannot be found, return an error
/// If the release cannot be found, return an error
#[cfg(feature = "ssr")]
pub async fn get_release_service(
    pool: &PgPool,
    user: Option<&User>,
    artist_slug: String,
    release_slug: String,
) -> Result<ReleaseResult, ServerFnError> {
    let artist = match Artist::get_by_slug(pool, artist_slug).await {
        Ok(artist) => artist,
        Err(e) => {
            let err = format!("Error while getting artist: {e:?}");
            tracing::error!("{err}");
            return Err(ServerFnError::new(e));
        }
    };

    let Some(current_user) = user else {
        return Err(ServerFnError::new("User not found"));
    };
    let include_hidden = current_user.permissions.contains("label_owner");

    let release = Release::get_by_artist_and_record_label_and_slug(pool, artist.id, artist.label_id, release_slug.clone(), include_hidden)
        .await
        .map_err(|e| {
            let err = format!("Error while getting artists: {e:?}");
            tracing::error!("{err}");
            ServerFnError::new(e)
        })?;
    let artists = release.get_artists(pool).await.map_err(|e| {
        let err = format!("Error while getting artists: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;

    Ok(ReleaseResult { release, artists })
}

/// Create a new release
///
/// # Arguments
/// pool: `PgPool` - The database connection pool
/// user: Option<&User> - The user creating the release
/// `form`: `CreateReleaseForm` - The form to create the release
///
/// # Returns
/// Result<`ReleaseResult`, `ServerFnError`> - The created release
///
/// # Errors
/// If the name is empty, return an error
/// If the release cannot be created, return an error
/// If the user does not have the required permissions, return an error
#[cfg(feature = "ssr")]
pub async fn create_release_service(pool: &PgPool, user: Option<&User>, form: CreateReleaseForm) -> Result<ReleaseResult, ServerFnError> {
    match user_with_permissions(user, vec!["admin", "label_owner"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }

    let release = Release::create(
        pool,
        form.name,
        form.description,
        form.primary_artist_id,
        form.catalogue_number,
        form.release_date,
        form.label_id,
        form.published_at,
    )
    .await
    .map_err(|e| {
        let err = format!("Error while creating release: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;

    let artist_ids = form.artist_ids.split(',').filter_map(|s| s.parse::<i64>().ok()).collect::<Vec<i64>>();
    release.set_artists(pool, artist_ids).await.map_err(|e| {
        let err = format!("Error while setting artists: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;

    let artists = release.get_artists(pool).await.map_err(|e| {
        let err = format!("Error while getting artists: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;

    Ok(ReleaseResult { release, artists })
}

/// Update a release
///
/// # Arguments
/// `pool`: `PgPool` - The database connection pool
/// `user`: `Option<&User>` - The user creating the release
/// `form`: `CreateReleaseForm` - The form to create the release
///
/// # Returns
/// Result<`ReleaseResult`, `ServerFnError`> - The created release
///
/// # Errors
/// If the name is empty, return an error
/// If the release cannot be created, return an error
/// If the user does not have the required permissions, return an error
#[cfg(feature = "ssr")]
pub async fn update_release_service(pool: &PgPool, user: Option<&User>, form: UpdateReleaseForm) -> Result<ReleaseResult, ServerFnError> {
    match user_with_permissions(user, vec!["admin", "label_owner"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }

    let mut release = Release::get_by_slug(pool, form.slug).await.map_err(|e| {
        let err = format!("Error while getting release by slug: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;

    release.name = form.name;
    release.description = form.description;
    release.primary_artist_id = form.primary_artist_id;
    release.catalogue_number = form.catalogue_number;
    release.release_date = form.release_date;
    release.published_at = form.published_at;

    release = release.update(pool).await.map_err(|e| {
        let err = format!("Error while updating release: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;

    let artist_ids = form.artist_ids.split(',').filter_map(|s| s.parse::<i64>().ok()).collect::<Vec<i64>>();
    release.set_artists(pool, artist_ids).await.map_err(|e| {
        let err = format!("Error while setting artists: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;
    let artists = release.get_artists(pool).await.map_err(|e| {
        let err = format!("Error while getting artists: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;

    Ok(ReleaseResult { release, artists })
}

/// Soft delete a release
///
/// # Arguments
/// pool: `PgPool` - The database connection pool
/// user: Option<&User> - The user deleting the release
/// slug: String - The slug of the release
///
/// # Returns
/// Result<`ReleaseResult`, `ServerFnError`> - The deleted release
///
/// # Errors
/// If the release cannot be found, return an error
/// If the user does not have the required permissions, return an error
#[cfg(feature = "ssr")]
pub async fn delete_release_service(pool: &PgPool, user: Option<&User>, slug: String) -> Result<ReleaseResult, ServerFnError> {
    match user_with_permissions(user, vec!["admin", "label_owner"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }

    let release = Release::get_by_slug(pool, slug).await.map_err(|e| {
        let err = format!("Error while getting release: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;

    Ok(ReleaseResult {
        release: release.delete(pool).await.map_err(|e| {
            let err = format!("Error while deleting release: {e:?}");
            tracing::error!("{err}");
            ServerFnError::new(e)
        })?,
        artists: release.get_artists(pool).await.map_err(|e| {
            let err = format!("Error while getting artists: {e:?}");
            tracing::error!("{err}");
            ServerFnError::new(e)
        })?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "ssr")]
    use crate::models::test_helpers::{
        create_test_artist, create_test_record_label, create_test_release, create_test_user, create_test_user_with_permissions,
    };

    #[sqlx::test]
    async fn test_get_releases_service_admin_user(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user: User = create_test_user_with_permissions(&pool, 1, permissions).await.unwrap();

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone())).await.unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone())).await.unwrap();
        let mut unpublished_release = create_test_release(&pool, 2, Some(artist.clone())).await.unwrap();
        unpublished_release.published_at = None;
        unpublished_release.release_date = None;
        unpublished_release.clone().update(&pool).await.unwrap();

        let releases = get_releases_service(&pool, Some(&user), artist.slug.clone()).await.unwrap();

        assert_eq!(releases.releases.len(), 2);
        assert_eq!(releases.releases[0].id, unpublished_release.id);
        assert_eq!(releases.releases[1].id, release.id);
    }

    #[sqlx::test]
    async fn test_get_releases_service_no_user(pool: PgPool) {
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone())).await.unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone())).await.unwrap();
        let mut unpublished_release = create_test_release(&pool, 2, Some(artist.clone())).await.unwrap();
        unpublished_release.published_at = None;
        unpublished_release.clone().update(&pool).await.unwrap();

        let releases = get_releases_service(&pool, None, artist.slug.clone()).await.unwrap();

        assert_eq!(releases.releases.len(), 1);
        assert_eq!(releases.releases[0].id, release.id);
    }

    #[sqlx::test]
    async fn test_get_releases_service_anonymous_user(pool: PgPool) {
        let (user, _) = create_test_user(&pool, 1).await.unwrap().into_user(None);

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone())).await.unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone())).await.unwrap();
        let mut unpublished_release = create_test_release(&pool, 2, Some(artist.clone())).await.unwrap();
        unpublished_release.published_at = None;
        unpublished_release.clone().update(&pool).await.unwrap();

        let releases = get_releases_service(&pool, Some(&user), artist.slug.clone()).await.unwrap();

        assert_eq!(releases.releases.len(), 1);
        assert_eq!(releases.releases[0].id, release.id);
    }

    #[sqlx::test]
    async fn test_get_release_service(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions).await.unwrap();

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone())).await.unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone())).await.unwrap();

        let release_result = get_release_service(&pool, Some(&user), artist.slug.clone(), release.slug.clone())
            .await
            .unwrap();

        assert_eq!(release_result.release.id, release.id);
    }

    #[sqlx::test]
    async fn test_get_release_service_no_permission(pool: PgPool) {
        let permissions = vec![];
        let user = create_test_user_with_permissions(&pool, 1, permissions).await.unwrap();

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone())).await.unwrap();
        let mut unpublished_release = create_test_release(&pool, 1, Some(artist.clone())).await.unwrap();
        unpublished_release.published_at = None;
        unpublished_release.clone().update(&pool).await.unwrap();

        let release_result = get_release_service(&pool, Some(&user), artist.slug.clone(), unpublished_release.slug.clone()).await;
        assert!(release_result.is_err());
        assert_eq!(
            release_result.unwrap_err().to_string(),
            "error running server function: Could not find release test-release-1 for artist with id 1 and record label with id 1.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_create_release_service(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user: User = create_test_user_with_permissions(&pool, 1, permissions).await.unwrap();

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone())).await.unwrap();

        let form = CreateReleaseForm {
            name: "Test Release".to_string(),
            description: "Test Release Description".to_string(),
            primary_artist_id: artist.id,
            catalogue_number: "TEST-123".to_string(),
            release_date: Some(chrono::Utc::now()),
            label_id: record_label.id,
            published_at: Some(chrono::Utc::now()),
            artist_ids: artist.id.to_string(),
        };

        let release_result = create_release_service(&pool, Some(&user), form.clone()).await.unwrap();

        assert_eq!(release_result.release.name, "Test Release");
        assert_eq!(release_result.release.description, "Test Release Description");
        assert_eq!(release_result.release.primary_artist_id, artist.id);
        assert_eq!(release_result.release.catalogue_number, "TEST-123");
        assert!(release_result.release.release_date.is_some());
        assert_eq!(release_result.release.label_id, record_label.id);
        assert!(release_result.release.published_at.is_some());
        assert_eq!(release_result.artists.len(), 1);
        assert_eq!(release_result.artists[0].id, artist.id);
    }

    #[sqlx::test]
    async fn test_create_release_service_no_permision(pool: PgPool) {
        let permissions = vec![];
        let user: User = create_test_user_with_permissions(&pool, 1, permissions).await.unwrap();

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone())).await.unwrap();

        let form = CreateReleaseForm {
            name: "Test Release".to_string(),
            description: "Test Release Description".to_string(),
            primary_artist_id: artist.id,
            catalogue_number: "TEST-123".to_string(),
            release_date: Some(chrono::Utc::now()),
            label_id: record_label.id,
            published_at: Some(chrono::Utc::now()),
            artist_ids: artist.id.to_string(),
        };

        let release_result = create_release_service(&pool, Some(&user), form).await;
        assert!(release_result.is_err());
        assert_eq!(
            release_result.unwrap_err().to_string(),
            "error running server function: You do not have permission.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_update_release_service_no_permission(pool: PgPool) {
        let permissions = vec![];
        let user: User = create_test_user_with_permissions(&pool, 1, permissions).await.unwrap();

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone())).await.unwrap();

        let form = CreateReleaseForm {
            name: "Test Release".to_string(),
            description: "Test Release Description".to_string(),
            primary_artist_id: artist.id,
            catalogue_number: "TEST-123".to_string(),
            release_date: Some(chrono::Utc::now()),
            label_id: record_label.id,
            published_at: Some(chrono::Utc::now()),
            artist_ids: artist.id.to_string(),
        };

        let release_result = create_release_service(&pool, Some(&user), form).await;
        assert!(release_result.is_err());
        assert_eq!(
            release_result.unwrap_err().to_string(),
            "error running server function: You do not have permission.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_update_release_service(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user: User = create_test_user_with_permissions(&pool, 1, permissions).await.unwrap();

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone())).await.unwrap();

        let form = CreateReleaseForm {
            name: "Test Release".to_string(),
            description: "Test Release Description".to_string(),
            catalogue_number: "TEST-123".to_string(),
            primary_artist_id: artist.id,
            release_date: Some(chrono::Utc::now()),
            label_id: record_label.id,
            published_at: Some(chrono::Utc::now()),
            artist_ids: artist.id.to_string(),
        };

        let release_result = create_release_service(&pool, Some(&user), form).await;
        assert!(release_result.is_ok());
        let release = release_result.unwrap();

        let update_form = UpdateReleaseForm {
            name: "Updated Release".to_string(),
            slug: "test-release".to_string(),
            description: "Updated Release Description".to_string(),
            primary_artist_id: artist.id,
            catalogue_number: "UPDATED-123".to_string(),
            release_date: Some(chrono::Utc::now()),
            label_id: record_label.id,
            published_at: Some(chrono::Utc::now()),
            artist_ids: artist.id.to_string(),
        };

        let update_result = update_release_service(&pool, Some(&user), update_form.clone()).await;
        assert!(update_result.is_ok());
        let updated_release = update_result.unwrap();

        assert_eq!(release.release.id, updated_release.release.id);
        assert_eq!(updated_release.release.name, "Updated Release");
        assert_eq!(updated_release.release.description, "Updated Release Description");
        assert_eq!(updated_release.release.primary_artist_id, artist.id);
        assert_eq!(updated_release.release.catalogue_number, "UPDATED-123");
        assert!(updated_release.release.release_date.is_some());
        assert_eq!(updated_release.release.label_id, record_label.id);
        assert!(updated_release.release.published_at.is_some());
        assert_eq!(updated_release.artists, vec![artist]);
    }

    #[sqlx::test]
    pub fn delete_release(pool: sqlx::PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user: User = create_test_user_with_permissions(&pool, 1, permissions).await.unwrap();

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone())).await.unwrap();

        let form = CreateReleaseForm {
            name: "Test Release".to_string(),
            description: "Test Release Description".to_string(),
            primary_artist_id: artist.id,
            catalogue_number: "TEST-123".to_string(),
            release_date: Some(chrono::Utc::now()),
            label_id: record_label.id,
            published_at: Some(chrono::Utc::now()),
            artist_ids: artist.id.to_string(),
        };

        let release_result = create_release_service(&pool, Some(&user), form).await;
        assert!(release_result.is_ok());
        let release = release_result.unwrap();

        let delete_result = delete_release_service(&pool, Some(&user), release.release.slug.clone()).await;
        assert!(delete_result.is_ok());

        let get_result = get_release_service(&pool, Some(&user), artist.slug, release.release.slug).await;
        assert!(get_result.is_ok());
        assert!(get_result.unwrap().release.deleted_at.is_some());
    }

    #[sqlx::test]
    pub fn delete_release_not_found(pool: sqlx::PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions).await.unwrap();

        let delete_result = delete_release_service(&pool, Some(&user), "not-found".to_string()).await;
        assert!(delete_result.is_err());
        assert_eq!(
            delete_result.unwrap_err().to_string(),
            "error running server function: Could not find release with slug not-found.".to_string()
        );
    }

    #[sqlx::test]
    pub fn delete_service_no_permission(pool: sqlx::PgPool) {
        let permissions = vec![];
        let user = create_test_user_with_permissions(&pool, 1, permissions).await.unwrap();

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone())).await.unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone())).await.unwrap();

        let delete_result = delete_release_service(&pool, Some(&user), release.slug.clone()).await;
        assert_eq!(
            delete_result.unwrap_err().to_string(),
            "error running server function: You do not have permission.".to_string()
        );

        let get_result = get_release_service(&pool, Some(&user), artist.slug, release.slug).await;
        assert!(get_result.is_ok());
    }
}
