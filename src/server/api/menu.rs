//! Admin-menu endpoint ported from `routes/menu.rs` (see the `api` module
//! docs for the wire protocol). Note: this route's server-fn args struct is
//! also called `GetRecordLabel` (same name as the one in
//! `routes::record_label`, different endpoint) — hence the import alias.

use axum::response::Response;

use crate::routes::menu::GetRecordLabel as AdminMenuArgs;
use crate::server::api::{WireError, decode_args, json_response};
use crate::server::current_user::{CurrentUser, Db};
use crate::services::menu::admin_menu as admin_menu_service;

const PATH_ADMIN_MENU: &str = "/api/admin_menu";

/// Fetch the nested record-label/artist/release/page data for the admin
/// menu (`admin` permission required — the service enforces it).
pub async fn admin_menu(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    decode_args::<AdminMenuArgs>(PATH_ADMIN_MENU, &body)?;

    let menu = admin_menu_service(&pool, current_user.as_ref())
        .await
        .map_err(|error| WireError::from_app_error(PATH_ADMIN_MENU, &error))?;

    json_response(PATH_ADMIN_MENU, &menu)
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::PATH_ADMIN_MENU;
    use crate::models::test_helpers::{
        create_test_artist, create_test_page, create_test_record_label, create_test_release,
        create_test_user_with_permissions,
    };
    use crate::routes::auth::Login;
    use crate::routes::menu::AdminMenu;
    use crate::server::api::test_support::{api_server, post_args};

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn admin_menu_without_login_is_unauthorized(pool: PgPool) {
        let server = api_server(pool).await;

        let response = post_args(&server, PATH_ADMIN_MENU, &()).await;

        response.assert_status_unauthorized();
        assert_eq!(response.text(), "ServerError|User not supplied.");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn admin_menu_without_admin_is_forbidden(pool: PgPool) {
        // `label_owner` alone is not enough — the service requires `admin`.
        create_test_user_with_permissions(&pool, 9, vec!["label_owner"])
            .await
            .expect("label_owner user");
        let server = api_server(pool).await;

        post_args(
            &server,
            "/api/login",
            &Login {
                username: "test-9".to_string(),
                password: "password".to_string(),
                remember: None,
            },
        )
        .await
        .assert_status_ok();

        let response = post_args(&server, PATH_ADMIN_MENU, &()).await;

        response.assert_status_forbidden();
        assert_eq!(response.text(), "ServerError|You do not have permission.");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn admin_menu_returns_the_nested_menu(pool: PgPool) {
        let label = create_test_record_label(&pool, 10).await.expect("label");
        let artist = create_test_artist(&pool, 10, Some(label.clone()))
            .await
            .expect("artist");
        create_test_release(&pool, 10, Some(artist))
            .await
            .expect("release");
        create_test_page(&pool, 10, Some(label.clone()))
            .await
            .expect("page");
        create_test_user_with_permissions(&pool, 10, vec!["admin"])
            .await
            .expect("admin user");
        let server = api_server(pool).await;

        post_args(
            &server,
            "/api/login",
            &Login {
                username: "test-10".to_string(),
                password: "password".to_string(),
                remember: None,
            },
        )
        .await
        .assert_status_ok();

        let response = post_args(&server, PATH_ADMIN_MENU, &()).await;

        response.assert_status_ok();
        let menu: AdminMenu = response.json();
        assert_eq!(menu.record_label.name, "Test Record Label 10");
        assert_ne!(menu.url, String::new());
        assert_eq!(menu.artists.len(), 1);
        assert_eq!(menu.artists[0].artist.slug, "test-artist-10");
        assert_eq!(menu.artists[0].releases.len(), 1);
        assert_eq!(menu.artists[0].releases[0].release.name, "Test Release 10");
        assert_eq!(menu.pages.len(), 1);
        assert_eq!(menu.pages[0].page.slug, "test-page-10");
    }
}
