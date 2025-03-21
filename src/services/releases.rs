//! Services for the releases routes
use leptos::prelude::ServerFnError;
use sqlx::PgPool;

use crate::models::{artist::Artist, auth::User, release::Release};
use crate::routes::releases::ReleasesResult;

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
}
