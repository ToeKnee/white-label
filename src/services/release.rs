//! Services for the releases routes
use leptos::prelude::ServerFnError;
use sqlx::PgPool;

use super::authentication_helpers::user_with_permissions;
use crate::forms::release::CreateReleaseForm;
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
pub async fn get_releases_service(
    pool: &PgPool,
    user: Option<&User>,
    slug: String,
) -> Result<ReleasesResult, ServerFnError> {
    let artist = match Artist::get_by_slug(pool, slug).await {
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

    Ok(ReleasesResult {
        releases: match Release::get_by_artist_and_record_label(
            pool,
            artist.id,
            artist.label_id,
            include_hidden,
        )
        .await
        {
            Ok(releases) => releases,
            Err(e) => {
                let err = format!("Error while getting releases: {e:?}");
                tracing::error!("{err}");
                return Err(ServerFnError::new(e));
            }
        },
    })
}

/// Create a new release
///
/// # Arguments
/// pool: `PgPool` - The database connection pool
/// user: Option<&User> - The user creating the release
/// `release_form`: `CreateReleaseForm` - The form to create the release
///
/// # Returns
/// Result<`ReleaseResult`, `ServerFnError`> - The created release
///
/// # Errors
/// If the name is empty, return an error
/// If the release cannot be created, return an error
/// If the user does not have the required permissions, return an error
#[cfg(feature = "ssr")]
pub async fn create_release_service(
    pool: &PgPool,
    user: Option<&User>,
    release_form: CreateReleaseForm,
) -> Result<ReleaseResult, ServerFnError> {
    match user_with_permissions(user, vec!["admin", "label_owner"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }

    let release = Release::create(
        pool,
        release_form.name,
        release_form.description,
        release_form.catalogue_number,
        release_form.release_date,
        release_form.label_id,
        release_form.published_at,
    )
    .await
    .map_err(|e| {
        let err = format!("Error while creating release: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;

    let artist_ids = release_form
        .artist_ids
        .split(',')
        .filter_map(|s| s.parse::<i64>().ok())
        .collect::<Vec<i64>>();
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

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "ssr")]
    use crate::models::test_helpers::{
        create_test_artist, create_test_record_label, create_test_release, create_test_user,
        create_test_user_with_permissions,
    };

    #[sqlx::test]
    async fn test_get_release_service_admin_user(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user: User = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();
        let mut unpublished_release = create_test_release(&pool, 2, Some(artist.clone()))
            .await
            .unwrap();
        unpublished_release.published_at = None;
        unpublished_release.clone().update(&pool).await.unwrap();

        let releases = get_releases_service(&pool, Some(&user), artist.slug.clone())
            .await
            .unwrap();

        assert_eq!(releases.releases.len(), 2);
        assert_eq!(releases.releases[0].id, release.id);
        assert_eq!(releases.releases[1].id, unpublished_release.id);
    }

    #[sqlx::test]
    async fn test_get_release_service_anonymous_user(pool: PgPool) {
        let (user, _) = create_test_user(&pool, 1).await.unwrap().into_user(None);

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();
        let mut unpublished_release = create_test_release(&pool, 2, Some(artist.clone()))
            .await
            .unwrap();
        unpublished_release.published_at = None;
        unpublished_release.clone().update(&pool).await.unwrap();

        let releases = get_releases_service(&pool, Some(&user), artist.slug.clone())
            .await
            .unwrap();

        assert_eq!(releases.releases.len(), 1);
        assert_eq!(releases.releases[0].id, release.id);
    }

    #[sqlx::test]
    async fn test_create_release_service(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user: User = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();

        let release_form = CreateReleaseForm {
            name: "Test Release".to_string(),
            description: "Test Release Description".to_string(),
            catalogue_number: "TEST-123".to_string(),
            release_date: Some(chrono::Utc::now()),
            label_id: record_label.id,
            published_at: Some(chrono::Utc::now()),
            artist_ids: artist.id.to_string(),
        };

        let release_result = create_release_service(&pool, Some(&user), release_form.clone())
            .await
            .unwrap();

        assert_eq!(release_result.release.name, "Test Release");
        assert_eq!(
            release_result.release.description,
            "Test Release Description"
        );
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
        let user: User = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();

        let release_form = CreateReleaseForm {
            name: "Test Release".to_string(),
            description: "Test Release Description".to_string(),
            catalogue_number: "TEST-123".to_string(),
            release_date: Some(chrono::Utc::now()),
            label_id: record_label.id,
            published_at: Some(chrono::Utc::now()),
            artist_ids: artist.id.to_string(),
        };

        let release_result = create_release_service(&pool, Some(&user), release_form).await;
        assert!(release_result.is_err());
        assert_eq!(
            release_result.unwrap_err().to_string(),
            "error running server function: You do not have permission.".to_string()
        );
    }
}
