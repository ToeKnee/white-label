//! Page endpoints ported from `routes/page.rs` (see the `api` module docs
//! for the wire protocol). Permission checks stay in the services, which
//! return [`AppError`]s that map straight onto wire statuses.

use axum::response::Response;

use crate::routes::page::{CreatePage, DeletePage, GetPage, RestorePage, UpdatePage};
use crate::server::api::{WireError, decode_args, json_response};
use crate::server::current_user::{CurrentUser, Db};
use crate::services::page::{
    create_page_service, delete_page_service, get_page_service, restore_page_service,
    update_page_service,
};

const PATH_GET_PAGE: &str = "/api/get_page";
const PATH_CREATE_PAGE: &str = "/api/create_page";
const PATH_UPDATE_PAGE: &str = "/api/update_page";
const PATH_DELETE_PAGE: &str = "/api/delete_page";
const PATH_RESTORE_PAGE: &str = "/api/restore_page";

/// Get a single page by slug. Public — but only published, undeleted pages
/// are visible without a session; `label_owner` users also see hidden ones
/// (the service decides via `CurrentUser`).
pub async fn get_page(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<GetPage>(PATH_GET_PAGE, &body)?;

    let result = get_page_service(&pool, current_user.as_ref(), args.slug)
        .await
        .map_err(|error| WireError::from_app_error(PATH_GET_PAGE, &error))?;

    json_response(PATH_GET_PAGE, &result)
}

/// Create a new page (admin / `label_owner`).
pub async fn create_page(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<CreatePage>(PATH_CREATE_PAGE, &body)?;

    let result = create_page_service(&pool, current_user.as_ref(), args.page_form)
        .await
        .map_err(|error| WireError::from_app_error(PATH_CREATE_PAGE, &error))?;

    json_response(PATH_CREATE_PAGE, &result)
}

/// Update an existing page (admin / `label_owner`).
pub async fn update_page(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<UpdatePage>(PATH_UPDATE_PAGE, &body)?;

    let result = update_page_service(&pool, current_user.as_ref(), args.page_form)
        .await
        .map_err(|error| WireError::from_app_error(PATH_UPDATE_PAGE, &error))?;

    json_response(PATH_UPDATE_PAGE, &result)
}

/// Soft-delete a page (admin / `label_owner`).
pub async fn delete_page(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<DeletePage>(PATH_DELETE_PAGE, &body)?;

    let result = delete_page_service(&pool, current_user.as_ref(), args.slug)
        .await
        .map_err(|error| WireError::from_app_error(PATH_DELETE_PAGE, &error))?;

    json_response(PATH_DELETE_PAGE, &result)
}

/// Restore a soft-deleted page (admin / `label_owner`).
pub async fn restore_page(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<RestorePage>(PATH_RESTORE_PAGE, &body)?;

    let result = restore_page_service(&pool, current_user.as_ref(), args.slug)
        .await
        .map_err(|error| WireError::from_app_error(PATH_RESTORE_PAGE, &error))?;

    json_response(PATH_RESTORE_PAGE, &result)
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::{
        PATH_CREATE_PAGE, PATH_DELETE_PAGE, PATH_GET_PAGE, PATH_RESTORE_PAGE, PATH_UPDATE_PAGE,
    };
    use crate::forms::page::{CreatePageForm, UpdatePageForm};
    use crate::models::test_helpers::{
        create_test_page, create_test_record_label, create_test_user_with_permissions,
    };
    use crate::routes::auth::Login;
    use crate::routes::page::{
        CreatePage, DeletePage, GetPage, PageResult, RestorePage, UpdatePage,
    };
    use crate::server::api::test_support::{api_server, post_args};

    /// Log the named test user (password "password") in through the API.
    fn login_args(username: &str) -> Login {
        Login {
            username: username.to_string(),
            password: "password".to_string(),
            remember: None,
        }
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn get_page_returns_the_published_page_json(pool: PgPool) {
        create_test_page(&pool, 1, None).await.expect("test page");
        let server = api_server(pool).await;

        let response = post_args(
            &server,
            PATH_GET_PAGE,
            &GetPage {
                slug: "test-page-1".to_string(),
            },
        )
        .await;

        response.assert_status_ok();
        let result: PageResult = response.json();
        assert_eq!(result.page.name, "Test Page 1");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn get_page_missing_is_internal(pool: PgPool) {
        // Today's behaviour: the model's not-found error surfaces as a 500
        // `ServerError|…`. Refining this to a 404 NotFound is follow-up.
        let server = api_server(pool).await;

        let response = post_args(
            &server,
            PATH_GET_PAGE,
            &GetPage {
                slug: "missing-page".to_string(),
            },
        )
        .await;

        response.assert_status_internal_server_error();
        assert_eq!(response.header("serverfnerror"), PATH_GET_PAGE);
        assert!(response.text().starts_with("ServerError|"));
        assert!(response.text().contains("missing-page"));
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn unpublished_page_hidden_from_anonymous_but_visible_to_label_owner(pool: PgPool) {
        let label = create_test_record_label(&pool, 5).await.expect("label");
        create_test_user_with_permissions(&pool, 5, vec!["admin", "label_owner"])
            .await
            .expect("admin + label_owner user");

        // Owner creates an unpublished page (published_at omitted → NULL).
        let owner = api_server(pool.clone()).await;
        post_args(&owner, "/api/login", &login_args("test-5"))
            .await
            .assert_status_ok();
        let created = post_args(
            &owner,
            PATH_CREATE_PAGE,
            &CreatePage {
                page_form: CreatePageForm {
                    name: "Draft Page".to_string(),
                    description: "not published yet".to_string(),
                    body: "# draft".to_string(),
                    label_id: label.id,
                    published_at: None,
                },
            },
        )
        .await;
        created.assert_status_ok();

        let get_args = GetPage {
            slug: "draft-page".to_string(),
        };

        // Anonymous sessions only see published, undeleted pages.
        let anonymous = api_server(pool).await;
        let hidden = post_args(&anonymous, PATH_GET_PAGE, &get_args).await;
        hidden.assert_status_internal_server_error();
        assert!(hidden.text().starts_with("ServerError|"));

        let visible = post_args(&owner, PATH_GET_PAGE, &get_args).await;
        visible.assert_status_ok();
        let result: PageResult = visible.json();
        assert_eq!(result.page.name, "Draft Page");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn create_page_without_login_is_unauthorized(pool: PgPool) {
        let server = api_server(pool).await;

        let response = post_args(
            &server,
            PATH_CREATE_PAGE,
            &CreatePage {
                page_form: CreatePageForm {
                    name: "No Session Page".to_string(),
                    description: String::new(),
                    body: String::new(),
                    label_id: 1,
                    published_at: None,
                },
            },
        )
        .await;

        response.assert_status_unauthorized();
        assert_eq!(response.text(), "ServerError|User not supplied.");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn create_page_without_permission_is_forbidden(pool: PgPool) {
        create_test_user_with_permissions(&pool, 6, vec![])
            .await
            .expect("permissionless user");
        let server = api_server(pool).await;

        post_args(&server, "/api/login", &login_args("test-6"))
            .await
            .assert_status_ok();

        let response = post_args(
            &server,
            PATH_CREATE_PAGE,
            &CreatePage {
                page_form: CreatePageForm {
                    name: "Unpermitted Page".to_string(),
                    description: String::new(),
                    body: String::new(),
                    label_id: 1,
                    published_at: None,
                },
            },
        )
        .await;

        response.assert_status_forbidden();
        assert_eq!(response.text(), "ServerError|You do not have permission.");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn label_owner_can_create_update_delete_and_restore(pool: PgPool) {
        let label = create_test_record_label(&pool, 7).await.expect("label");
        // The page services require both tokens (user_with_permissions
        // checks every listed permission) — same as the service tests.
        create_test_user_with_permissions(&pool, 7, vec!["admin", "label_owner"])
            .await
            .expect("admin + label_owner user");
        let server = api_server(pool.clone()).await;

        post_args(&server, "/api/login", &login_args("test-7"))
            .await
            .assert_status_ok();

        let created = post_args(
            &server,
            PATH_CREATE_PAGE,
            &CreatePage {
                page_form: CreatePageForm {
                    name: "About Us".to_string(),
                    description: "the story".to_string(),
                    body: "# About".to_string(),
                    label_id: label.id,
                    published_at: None,
                },
            },
        )
        .await;
        created.assert_status_ok();
        let created: PageResult = created.json();
        assert_eq!(created.page.slug, "about-us");
        assert_eq!(created.page.label_id, label.id);

        let updated = post_args(
            &server,
            PATH_UPDATE_PAGE,
            &UpdatePage {
                page_form: UpdatePageForm {
                    slug: "about-us".to_string(),
                    name: "About Our Label".to_string(),
                    description: "still here".to_string(),
                    body: "# About v2".to_string(),
                    published_at: None,
                },
            },
        )
        .await;
        updated.assert_status_ok();
        let updated: PageResult = updated.json();
        assert_eq!(updated.page.name, "About Our Label");
        // `Page::update` re-slugifies from the name (existing model
        // behaviour), so later calls address the new slug.
        assert_eq!(updated.page.slug, "about-our-label");

        let deleted = post_args(
            &server,
            PATH_DELETE_PAGE,
            &DeletePage {
                slug: "about-our-label".to_string(),
            },
        )
        .await;
        deleted.assert_status_ok();
        let deleted: PageResult = deleted.json();
        assert!(deleted.page.deleted_at.is_some());

        // A soft-deleted page is invisible to anonymous visitors…
        let anonymous = api_server(pool.clone()).await;
        let after_delete = post_args(
            &anonymous,
            PATH_GET_PAGE,
            &GetPage {
                slug: "about-our-label".to_string(),
            },
        )
        .await;
        after_delete.assert_status_internal_server_error();

        // …while the owner can restore it.
        let restored = post_args(
            &server,
            PATH_RESTORE_PAGE,
            &RestorePage {
                slug: "about-our-label".to_string(),
            },
        )
        .await;
        restored.assert_status_ok();
        let restored: PageResult = restored.json();
        assert!(restored.page.deleted_at.is_none());

        let after_restore = post_args(
            &anonymous,
            PATH_GET_PAGE,
            &GetPage {
                slug: "about-our-label".to_string(),
            },
        )
        .await;
        after_restore.assert_status_internal_server_error();
        // Restoring only clears `deleted_at`; the page stays unpublished
        // (published_at is still NULL), so it remains hidden from the
        // anonymous session.
        let owner_view = post_args(
            &server,
            PATH_GET_PAGE,
            &GetPage {
                slug: "about-our-label".to_string(),
            },
        )
        .await;
        owner_view.assert_status_ok();
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn create_page_with_empty_name_is_internal(pool: PgPool) {
        // Validation runs in the model and is indistinguishable from a DB
        // failure here, so it surfaces as a 500 (see the Phase 2 error
        // mapping policy; refining this is follow-up work).
        let label = create_test_record_label(&pool, 8).await.expect("label");
        create_test_user_with_permissions(&pool, 8, vec!["admin", "label_owner"])
            .await
            .expect("admin + label_owner user");
        let server = api_server(pool).await;

        post_args(&server, "/api/login", &login_args("test-8"))
            .await
            .assert_status_ok();

        let response = post_args(
            &server,
            PATH_CREATE_PAGE,
            &CreatePage {
                page_form: CreatePageForm {
                    name: String::new(),
                    description: String::new(),
                    body: String::new(),
                    label_id: label.id,
                    published_at: None,
                },
            },
        )
        .await;

        response.assert_status_internal_server_error();
        assert!(response.text().starts_with("ServerError|"));
        assert!(response.text().contains("Name is required."));
    }
}
