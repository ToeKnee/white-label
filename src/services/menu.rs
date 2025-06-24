//! This service will handle the menu operations, such as fetching the menu items and their details.
use leptos::prelude::ServerFnError;
use sqlx::PgPool;

use crate::models::{
    artist::Artist, auth::User, page::Page, record_label::RecordLabel, release::Release,
};
#[cfg(feature = "ssr")]
use crate::routes::menu::{AdminMenu, MenuArtist, MenuPage, MenuRelease};
#[cfg(feature = "ssr")]
use crate::services::authentication_helpers::user_with_permissions;

#[cfg(feature = "ssr")]
/// Fetches the admin menu data, including record label, artists, releases, and tracks.
/// This function requires the user to have admin permissions.
///
/// # Arguments
/// * `pool`: A reference to the database connection pool.
/// * `user`: An optional reference to the user requesting the menu.
///
/// # Returns
/// A `Result` containing an `AdminMenu` struct on success, or a `ServerFnError` on failure.
///
/// # Errors
/// Will return a `ServerFnError` if the user does not have the required permissions.
/// Will return a `ServerFnError` if there is an issue retrieving any of the data (record label, artists, releases, tracks).
pub async fn admin_menu(pool: &PgPool, user: Option<&User>) -> Result<AdminMenu, ServerFnError> {
    match user_with_permissions(user, vec!["admin"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }
    let include_hidden = true;

    let record_label = RecordLabel::first(pool).await.map_err(|x| {
        tracing::error!("Error while getting record label: {x:?}");
        ServerFnError::new("Could not retrieve record label, try again later")
    })?;

    let artists = Artist::list_by_record_label(pool, record_label.id, include_hidden)
        .await
        .map_err(|x| {
            tracing::error!("Error while getting artists: {x:?}");
            ServerFnError::new("Could not retrieve artists, try again later")
        })?;

    let mut menu_artists = Vec::new();
    for artist in artists {
        let releases = Release::list_by_artist_and_record_label(
            pool,
            artist.id,
            record_label.id,
            include_hidden,
        )
        .await
        .map_err(|x| {
            tracing::error!("Error while getting releases: {x:?}");
            ServerFnError::new("Could not retrieve releases, try again later")
        })?;

        let mut menu_releases = Vec::new();
        for release in releases {
            menu_releases.push(MenuRelease {
                release: release.clone(),
                url: format!("/admin/artist/{}/release/{}", artist.slug, release.slug),
            });
        }

        menu_artists.push(MenuArtist {
            artist: artist.clone(),
            url: format!("/admin/artist/{}", artist.slug),
            releases: menu_releases,
        });
    }

    let pages = Page::list(pool, include_hidden).await.map_err(|x| {
        tracing::error!("Error while getting pages: {x:?}");
        ServerFnError::new("Could not retrieve pages, try again later")
    })?;
    let menu_pages = pages
        .into_iter()
        .map(|page| MenuPage {
            page: page.clone(),
            url: format!("/admin/page/{}", page.slug),
        })
        .collect();

    Ok(AdminMenu {
        record_label,
        url: "/admin/label".to_string(),
        artists: menu_artists,
        pages: menu_pages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "ssr")]
    use crate::models::test_helpers::{
        create_test_artist, create_test_page, create_test_record_label, create_test_release,
        create_test_user_with_permissions,
    };

    #[sqlx::test]
    async fn test_admin_menu(pool: PgPool) {
        let permissions = vec!["admin"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();
        let record_label = create_test_record_label(&pool, 1).await.unwrap();
        let artist = create_test_artist(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();
        let release = create_test_release(&pool, 1, Some(artist.clone()))
            .await
            .unwrap();
        let page = create_test_page(&pool, 1, Some(record_label.clone()))
            .await
            .unwrap();

        let menu = admin_menu(&pool, Some(&user)).await.unwrap();

        assert_eq!(menu.record_label.id, record_label.id);
        assert_eq!(menu.artists.len(), 1);
        assert_eq!(menu.artists[0].artist.id, artist.id);
        assert_eq!(menu.artists[0].releases.len(), 1);
        assert_eq!(menu.artists[0].releases[0].release.id, release.id);
        assert_eq!(menu.pages.len(), 1);
        assert_eq!(menu.pages[0].page.id, page.id);
    }
}
