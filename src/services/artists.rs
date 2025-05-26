//! Services that act on multiple artists at a time
use futures::{StreamExt, TryStreamExt};
use itertools::Itertools;
use leptos::prelude::ServerFnError;
use sqlx::PgPool;

use super::authentication_helpers::user_with_permissions;
use crate::models::release::Release;
use crate::models::{artist::Artist, auth::User};

/// Get releases by artist ids
///
/// # Arguments
/// pool: `PgPool` - The database connection pool
/// user: `Option<&User>` - The user making the request
/// `artist_ids`: `Vec<i64>` - The artist ids
///
/// # Returns
/// Result<`ArtistResult`, `ServerFnError`> - The artist
///
/// # Errors
/// If the artist cannot be found, return an error
pub async fn get_releases_for_artists_service(
    pool: &PgPool,
    user: Option<&User>,
    artist_ids: Vec<i64>,
) -> Result<Vec<Release>, ServerFnError> {
    match user_with_permissions(user, vec!["admin", "label_owner"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }
    let include_hidden = user.is_some_and(|user| user.permissions.contains("label_owner"));

    let artists = artist_ids
        .iter()
        .map(|artist_id| async {
            Artist::get_by_id(pool, *artist_id).await.map_err(|x| {
                let err = format!("Error while getting artist: {x:?}");
                tracing::error!("{err}");
                ServerFnError::new("Could not retrieve artist, try again later")
            })
        })
        .collect::<futures::stream::FuturesUnordered<_>>()
        .try_collect::<Vec<Artist>>()
        .await?;

    let releases = artists
        .into_iter()
        .map(|artist| async move {
            Release::list_by_artist_and_record_label(
                pool,
                artist.id,
                artist.label_id,
                include_hidden,
            )
            .await
            .map_err(|x| {
                let err = format!(
                    "Error while getting releases for artist {}: {x:?}",
                    artist.name
                );
                tracing::error!("{err}");
                ServerFnError::new("Could not retrieve releases, try again later")
            })
        })
        .collect::<futures::stream::FuturesUnordered<_>>()
        .collect::<Vec<Result<Vec<Release>, ServerFnError>>>();
    // Flatten, sort and deduplicate the the results
    let releases = releases
        .await
        .into_iter()
        .filter_map(std::result::Result::ok)
        .flatten()
        .unique()
        .sorted_by(|a, b| a.release_date.cmp(&b.release_date))
        .rev()
        .collect::<Vec<_>>();
    Ok(releases)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "ssr")]
    use crate::models::test_helpers::{
        create_test_artist, create_test_record_label, create_test_release,
        create_test_user_with_permissions,
    };

    #[sqlx::test]
    async fn test_get_releases_for_artists(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label))
            .await
            .unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();

        let releases = get_releases_for_artists_service(&pool, Some(&user), vec![artist.id])
            .await
            .unwrap();

        assert_eq!(releases.len(), 1);
        assert_eq!(releases[0].id, release.id);
    }

    #[sqlx::test]
    async fn test_get_releases_for_artists_no_permission(pool: PgPool) {
        let permissions = vec![];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label))
            .await
            .unwrap();
        let _release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();

        let result = get_releases_for_artists_service(&pool, Some(&user), vec![artist.id]).await;

        assert!(result.is_err());
    }

    #[sqlx::test]
    async fn test_get_releases_for_artists_multiple_artists(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist1 = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();
        let artist2 = create_test_artist(&pool, 2, Some(record_label))
            .await
            .unwrap();
        let release1 = create_test_release(&pool, 1, Some(artist1.clone()))
            .await
            .unwrap();
        let release2 = create_test_release(&pool, 2, Some(artist2.clone()))
            .await
            .unwrap();

        let releases =
            get_releases_for_artists_service(&pool, Some(&user), vec![artist1.id, artist2.id])
                .await
                .unwrap();

        assert_eq!(releases.len(), 2);
        assert_eq!(releases[0].id, release2.id);
        assert_eq!(releases[1].id, release1.id);
    }
}
