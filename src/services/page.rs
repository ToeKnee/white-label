//! Services for the page routes
use leptos::prelude::ServerFnError;
use sqlx::PgPool;

use super::authentication_helpers::user_with_permissions;
use crate::forms::page::{CreatePageForm, UpdatePageForm};
use crate::models::{auth::User, page::Page};
use crate::routes::page::PageResult;

/// Get an page by slug
///
/// # Arguments
/// pool: `PgPool` - The database connection pool
/// user: Option<&User> - The user creating the artist
/// slug: String - The slug of the page
///
/// # Returns
/// Result<`PageResult`, `ServerFnError`> - The page
///
/// # Errors
/// If the page cannot be found, return an error
pub async fn get_page_service(
    pool: &PgPool,
    user: Option<&User>,
    slug: String,
) -> Result<PageResult, ServerFnError> {
    let include_hidden = user.is_some_and(|user| user.permissions.contains("label_owner"));
    Ok(PageResult {
        page: Page::get_by_slug(pool, slug, include_hidden)
            .await
            .map_err(|e| {
                let err = format!("Error while getting page: {e:?}");
                tracing::error!("{err}");
                ServerFnError::new(e)
            })?,
    })
}

/// Restore a soft deleted page
///
/// # Arguments
/// pool: `PgPool` - The database connection pool
/// user: Option<&User> - The user deleting the page
/// slug: String - The slug of the page
///
/// # Returns
/// Result<`PageResult`, `ServerFnError`> - The restored page
///
/// # Errors
/// If the page cannot be found, return an error
/// If the user does not have the required permissions, return an error
#[cfg(feature = "ssr")]
pub async fn restore_page_service(
    pool: &PgPool,
    user: Option<&User>,
    slug: String,
) -> Result<PageResult, ServerFnError> {
    match user_with_permissions(user, vec!["admin", "label_owner"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }
    let include_hidden = true; // We have already checked permissions, so we can include hidden pages

    let mut page = Page::get_by_slug(pool, slug, include_hidden)
        .await
        .map_err(|e| {
            let err = format!("Error while getting page: {e:?}");
            tracing::error!("{err}");
            ServerFnError::new(e)
        })?;
    page.deleted_at = None;
    page.clone().update(pool).await.map_err(|e| {
        let err = format!("Error while restoring page: {e:?}");
        tracing::error!("{err}");
        ServerFnError::new(e)
    })?;

    Ok(PageResult { page })
}

/// Create a new page
///
/// # Arguments
/// pool: `PgPool` - The database connection pool
/// user: Option<&User> - The user creating the page
/// `page_form`: `CreatePageForm` - The form to create the page
///
/// # Returns
/// Result<`PageResult`, `ServerFnError`> - The created page
///
/// # Errors
/// If the name is empty, return an error
/// If the page cannot be created, return an error
/// If the user does not have the required permissions, return an error
#[cfg(feature = "ssr")]
pub async fn create_page_service(
    pool: &PgPool,
    user: Option<&User>,
    page_form: CreatePageForm,
) -> Result<PageResult, ServerFnError> {
    match user_with_permissions(user, vec!["admin", "label_owner"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }

    Ok(PageResult {
        page: Page::create(
            pool,
            page_form.name,
            page_form.description,
            page_form.body,
            page_form.label_id,
            page_form.published_at,
        )
        .await
        .map_err(|e| {
            let err = format!("Error while creating page: {e:?}");
            tracing::error!("{err}");
            ServerFnError::new(e)
        })?,
    })
}

/// Update an page
///
/// # Arguments
/// pool: `PgPool` - The database connection pool
/// user: Option<&User> - The user updating the page
/// `page_form`: `UpdatePageForm` - The form to update the page
///
/// # Returns
/// Result<`PageResult`, `ServerFnError`> - The updated page
///
/// # Errors
/// If the name is empty, return an error
/// If the page cannot be updated, return an error
/// If the user does not have the required permissions, return an error
#[cfg(feature = "ssr")]
pub async fn update_page_service(
    pool: &PgPool,
    user: Option<&User>,
    page_form: UpdatePageForm,
) -> Result<PageResult, ServerFnError> {
    match user_with_permissions(user, vec!["admin", "label_owner"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }
    let include_hidden = true; // We have already checked permissions, so we can include hidden pages

    let mut page = Page::get_by_slug(pool, page_form.slug, include_hidden)
        .await
        .map_err(|e| {
            let err = format!("Error while getting page: {e:?}");
            tracing::error!("{err}");
            ServerFnError::new(e)
        })?;
    page.name = page_form.name;
    page.description = page_form.description;
    page.body = page_form.body;
    page.published_at = page_form.published_at;

    Ok(PageResult {
        page: page.update(pool).await.map_err(|e| {
            let err = format!("Error while updating page: {e:?}");
            tracing::error!("{err}");
            ServerFnError::new(e)
        })?,
    })
}

/// Soft delete an page
///
/// # Arguments
/// pool: `PgPool` - The database connection pool
/// user: Option<&User> - The user deleting the page
/// slug: String - The slug of the page
///
/// # Returns
/// Result<`PageResult`, `ServerFnError`> - The deleted page
///
/// # Errors
/// If the page cannot be found, return an error
/// If the user does not have the required permissions, return an error
#[cfg(feature = "ssr")]
pub async fn delete_page_service(
    pool: &PgPool,
    user: Option<&User>,
    slug: String,
) -> Result<PageResult, ServerFnError> {
    match user_with_permissions(user, vec!["admin", "label_owner"]) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }
    let include_hidden = true; // We have already checked permissions, so we can include hidden pages

    let page = Page::get_by_slug(pool, slug, include_hidden)
        .await
        .map_err(|e| {
            let err = format!("Error while getting page: {e:?}");
            tracing::error!("{err}");
            ServerFnError::new(e)
        })?;

    Ok(PageResult {
        page: page.delete(pool).await.map_err(|e| {
            let err = format!("Error while deleting page: {e:?}");
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
        create_test_page, create_test_record_label, create_test_user_with_permissions,
    };

    #[sqlx::test]
    async fn test_get_page_service(pool: PgPool) {
        let page = create_test_page(&pool, 1, None).await.unwrap();
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();
        let page_by_slug = get_page_service(&pool, Some(&user), page.slug.clone())
            .await
            .unwrap();
        assert_eq!(page, page_by_slug.page);
    }

    #[sqlx::test]
    async fn test_get_page_service_hidden_page_admin(pool: PgPool) {
        let mut page = create_test_page(&pool, 1, None).await.unwrap();
        page.deleted_at = Some(chrono::Utc::now());
        page = page.clone().update(&pool).await.unwrap();
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();
        let page_by_slug = get_page_service(&pool, Some(&user), page.slug.clone())
            .await
            .unwrap();
        assert_eq!(page, page_by_slug.page);
    }
    #[sqlx::test]
    async fn test_get_page_service_hidden_page_not_admin(pool: PgPool) {
        let mut page = create_test_page(&pool, 1, None).await.unwrap();
        page.deleted_at = Some(chrono::Utc::now());
        page.clone().update(&pool).await.unwrap();
        let permissions = vec![];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();
        let result = get_page_service(&pool, Some(&user), "missing".to_string()).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "error running server function: Could not find page with slug missing.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_get_page_service_no_page(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();
        let result = get_page_service(&pool, Some(&user), "missing".to_string()).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "error running server function: Could not find page with slug missing.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_create_page_service(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();
        let record_label = create_test_record_label(&pool, 1).await.unwrap();

        let page_form = CreatePageForm {
            name: "Test Page".to_string(),
            description: "This is a test page".to_string(),
            body: "# This is an updated page".to_string(),
            label_id: record_label.id,
            published_at: None,
        };

        let page = create_page_service(&pool, Some(&user), page_form)
            .await
            .unwrap();
        assert_eq!(page.page.name, "Test Page".to_string());
        assert_eq!(page.page.description, "This is a test page".to_string());
    }

    #[sqlx::test]
    async fn test_create_page_service_no_permission(pool: PgPool) {
        let user = create_test_user_with_permissions(&pool, 1, vec!["admin"]) // but not label_owner
            .await
            .unwrap();
        let record_label = create_test_record_label(&pool, 1).await.unwrap();

        let page_form = CreatePageForm {
            name: "Test Page".to_string(),
            description: "This is a test page".to_string(),
            body: "# This is an updated page".to_string(),
            label_id: record_label.id,
            published_at: None,
        };

        let page = create_page_service(&pool, Some(&user), page_form).await;

        assert!(page.is_err());
        assert_eq!(
            page.unwrap_err().to_string(),
            "error running server function: You do not have permission.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_create_page_service_no_name(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();
        let record_label = create_test_record_label(&pool, 1).await.unwrap();

        let page_form = CreatePageForm {
            name: String::new(),
            description: "This is a test page".to_string(),
            body: "# This is an updated page".to_string(),
            label_id: record_label.id,
            published_at: None,
        };

        let page = create_page_service(&pool, Some(&user), page_form).await;

        assert!(page.is_err());
        assert_eq!(
            page.unwrap_err().to_string(),
            "error running server function: Name is required.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_create_page_service_name_too_long(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();
        let record_label = create_test_record_label(&pool, 1).await.unwrap();

        let name = "a".repeat(256);
        let page_form = CreatePageForm {
            name,
            description: "This is a test page".to_string(),
            body: "# This is an updated page".to_string(),
            label_id: record_label.id,
            published_at: None,
        };
        let page = create_page_service(&pool, Some(&user), page_form).await;

        assert!(page.is_err());
        assert_eq!(
            page.unwrap_err().to_string(),
            "error running server function: Name must be less than 255 characters.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_create_page_service_no_record_label(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let page_form = CreatePageForm {
            name: "Test Page".to_string(),
            description: "This is a test page".to_string(),
            body: "# This is an updated page".to_string(),
            label_id: 0,
            published_at: None,
        };

        let page = create_page_service(&pool, Some(&user), page_form).await;

        assert!(page.is_err());
        assert_eq!(
            page.unwrap_err().to_string(),
            "error running server function: Record Label with id 0 does not exist.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_update_page_service(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let page = create_test_page(&pool, 1, None).await.unwrap();
        let page_form = UpdatePageForm {
            slug: page.slug.clone(),
            name: "Updated Page".to_string(),
            description: "This is an updated page".to_string(),
            body: "# This is an updated page".to_string(),
            published_at: Some(chrono::Utc::now()),
        };
        let updated_page = update_page_service(&pool, Some(&user), page_form)
            .await
            .unwrap();
        assert_eq!(updated_page.page.name, "Updated Page".to_string());
        assert_eq!(
            updated_page.page.description,
            "This is an updated page".to_string()
        );
    }

    #[sqlx::test]
    async fn test_update_page_service_name_is_empty(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let page = create_test_page(&pool, 1, None).await.unwrap();
        let page_form = UpdatePageForm {
            slug: page.slug.clone(),
            name: String::new(),
            description: "This is an updated page".to_string(),
            body: "# This is an updated page".to_string(),
            published_at: Some(chrono::Utc::now()),
        };
        let updated_page = update_page_service(&pool, Some(&user), page_form).await;

        assert!(updated_page.is_err());
        assert_eq!(
            updated_page.unwrap_err().to_string(),
            "error running server function: Name is required.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_update_page_service_name_too_long(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let name = "a".repeat(256);
        let page = create_test_page(&pool, 1, None).await.unwrap();
        let page_form = UpdatePageForm {
            slug: page.slug.clone(),
            name,
            description: "This is an updated page".to_string(),
            body: "# This is an updated page".to_string(),
            published_at: Some(chrono::Utc::now()),
        };
        let updated_page = update_page_service(&pool, Some(&user), page_form).await;

        assert!(updated_page.is_err());
        assert_eq!(
            updated_page.unwrap_err().to_string(),
            "error running server function: Name must be less than 255 characters.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_update_page_service_no_page(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let page_form = UpdatePageForm {
            slug: "missing".to_string(),
            name: "Updated Page".to_string(),
            description: "This is an updated page".to_string(),
            body: "# This is an updated page".to_string(),
            published_at: Some(chrono::Utc::now()),
        };
        let updated_page = update_page_service(&pool, Some(&user), page_form).await;

        assert!(updated_page.is_err());
        assert_eq!(
            updated_page.unwrap_err().to_string(),
            "error running server function: Could not find page with slug missing.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_update_page_service_no_user(pool: PgPool) {
        let page = create_test_page(&pool, 1, None).await.unwrap();
        let page_form = UpdatePageForm {
            slug: page.slug.clone(),
            name: "Updated Page".to_string(),
            description: "This is an updated page".to_string(),
            body: "# This is an updated page".to_string(),
            published_at: Some(chrono::Utc::now()),
        };
        let updated_page = update_page_service(&pool, Some(&User::default()), page_form).await;

        assert!(updated_page.is_err());
        assert_eq!(
            updated_page.unwrap_err().to_string(),
            "error running server function: You must be logged in to view this page.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_update_page_service_no_permissions(pool: PgPool) {
        let user = create_test_user_with_permissions(&pool, 1, vec![])
            .await
            .unwrap();
        let page = create_test_page(&pool, 1, None).await.unwrap();
        let page_form = UpdatePageForm {
            slug: page.slug.clone(),
            name: "Updated Page".to_string(),
            description: "This is an updated page".to_string(),
            body: "# This is an updated page".to_string(),
            published_at: Some(chrono::Utc::now()),
        };
        let updated_page = update_page_service(&pool, Some(&user), page_form).await;

        assert!(updated_page.is_err());
        assert_eq!(
            updated_page.unwrap_err().to_string(),
            "error running server function: You do not have permission.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_delete_page_service(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let page = create_test_page(&pool, 1, None).await.unwrap();
        let deleted_page = delete_page_service(&pool, Some(&user), page.slug.clone())
            .await
            .unwrap();
        assert!(deleted_page.page.deleted_at.is_some());
    }

    #[sqlx::test]
    async fn test_delete_page_service_no_page(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let deleted_page = delete_page_service(&pool, Some(&user), "missing".to_string()).await;
        assert!(deleted_page.is_err());
        assert_eq!(
            deleted_page.unwrap_err().to_string(),
            "error running server function: Could not find page with slug missing.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_delete_page_service_no_user(pool: PgPool) {
        let page = create_test_page(&pool, 1, None).await.unwrap();
        let deleted_page =
            delete_page_service(&pool, Some(&User::default()), page.slug.clone()).await;
        assert!(deleted_page.is_err());
        assert_eq!(
            deleted_page.unwrap_err().to_string(),
            "error running server function: You must be logged in to view this page.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_delete_page_service_no_permissions(pool: PgPool) {
        let user = create_test_user_with_permissions(&pool, 1, vec![])
            .await
            .unwrap();
        let page = create_test_page(&pool, 1, None).await.unwrap();
        let deleted_page = delete_page_service(&pool, Some(&user), page.slug.clone()).await;
        assert!(deleted_page.is_err());
        assert_eq!(
            deleted_page.unwrap_err().to_string(),
            "error running server function: You do not have permission.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_restore_page_service(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let page = create_test_page(&pool, 1, None).await.unwrap();
        let page = page.delete(&pool).await.unwrap();
        let restored_page = restore_page_service(&pool, Some(&user), page.slug.clone())
            .await
            .unwrap();

        assert!(restored_page.page.deleted_at.is_none());
    }

    #[sqlx::test]
    async fn test_restore_page_service_no_page(pool: PgPool) {
        let permissions = vec!["admin", "label_owner"];
        let user = create_test_user_with_permissions(&pool, 1, permissions)
            .await
            .unwrap();

        let restored_page = restore_page_service(&pool, Some(&user), "missing".to_string()).await;
        assert!(restored_page.is_err());
        assert_eq!(
            restored_page.unwrap_err().to_string(),
            "error running server function: Could not find page with slug missing.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_restore_page_service_no_user(pool: PgPool) {
        let page = create_test_page(&pool, 1, None).await.unwrap();
        let page = page.delete(&pool).await.unwrap();
        let restored_page =
            restore_page_service(&pool, Some(&User::default()), page.slug.clone()).await;
        assert!(restored_page.is_err());
        assert_eq!(
            restored_page.unwrap_err().to_string(),
            "error running server function: You must be logged in to view this page.".to_string()
        );
    }

    #[sqlx::test]
    async fn test_restore_page_service_no_permissions(pool: PgPool) {
        let user = create_test_user_with_permissions(&pool, 1, vec![])
            .await
            .unwrap();
        let page = create_test_page(&pool, 1, None).await.unwrap();
        let page = page.delete(&pool).await.unwrap();
        let restored_page = restore_page_service(&pool, Some(&user), page.slug.clone()).await;
        assert!(restored_page.is_err());
        assert_eq!(
            restored_page.unwrap_err().to_string(),
            "error running server function: You do not have permission.".to_string()
        );
    }
}
