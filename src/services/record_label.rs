//! Services for record-label operations.
//!
//! Both label editors — the no-JS handler in `crate::server::forms` and the
//! Dioxus server fn in `crate::ui::admin::label` — call
//! [`update_record_label`], so the validation and error messages live in
//! one place.
use crate::error::AppError;
use sqlx::PgPool;

use crate::models::auth::User;
use crate::models::record_label::RecordLabel;

/// Update the record label's name, description and ISRC base.
/// `label_owner` only; `update` also re-slugifies from the (new) name.
///
/// # Arguments
/// pool: `PgPool` - The database connection pool
/// user: Option<&User> - The session user
/// id: i64 - The label id to update
/// name: String - The new label name
/// description: String - The new description (markdown)
/// `isrc_base`: `String` - The new ISRC prefix
/// `guest_plays`: `i32` - The new guest play allowance (already parsed and
/// clamped by [`crate::forms::record_label::LabelFields::parsed_guest_plays`])
///
/// # Errors
/// `Unauthorized` when there is no session user, `Forbidden` without the
/// `label_owner` permission, `Internal` with the fixed retry messages when
/// the label cannot be loaded or saved (details in the tracing log).
#[cfg(feature = "server")]
pub async fn update_record_label(
    pool: &PgPool,
    user: Option<&User>,
    id: i64,
    name: String,
    description: String,
    isrc_base: String,
    guest_plays: i32,
) -> Result<RecordLabel, AppError> {
    let current_user = user.ok_or_else(|| {
        tracing::error!("update_record_label: no session user");
        AppError::Unauthorized("You must be logged in to update a label".into())
    })?;
    if !current_user.permissions.contains("label_owner") {
        tracing::error!("update_record_label: user without label_owner");
        return Err(AppError::Forbidden(
            "You do not have permission to update a label".into(),
        ));
    }

    let mut record_label = RecordLabel::get_by_id(pool, id).await.map_err(|e| {
        tracing::error!("update_record_label: Error while getting label: {e:?}");
        AppError::Internal("Could not retrieve label, try again later".into())
    })?;

    record_label.name = name;
    record_label.description = description;
    record_label.isrc_base = isrc_base;
    record_label.guest_plays = guest_plays;
    record_label.update(pool).await.map_err(|e| {
        tracing::error!("update_record_label: Error while updating label: {e}");
        AppError::Internal("Could not update label, try again later".into())
    })
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;
    use crate::models::test_helpers::{
        create_test_record_label, create_test_user_with_permissions,
    };

    #[sqlx::test]
    async fn update_requires_a_session_user(pool: PgPool) {
        let error =
            update_record_label(&pool, None, 1, "x".into(), String::new(), String::new(), 5)
                .await
                .expect_err("guests must not update labels");
        assert_eq!(
            error,
            AppError::Unauthorized("You must be logged in to update a label".into())
        );
    }

    #[sqlx::test]
    async fn update_requires_label_owner(pool: PgPool) {
        // A plain `admin` — or a signed-in user with no permissions — is
        // not enough; the check is `label_owner`.
        let admin = create_test_user_with_permissions(&pool, 1, vec!["admin"])
            .await
            .expect("admin user");
        let plain = create_test_user_with_permissions(&pool, 2, vec![])
            .await
            .expect("plain user");

        for user in [admin, plain.clone()] {
            let error = update_record_label(
                &pool,
                Some(&user),
                1,
                "x".into(),
                String::new(),
                String::new(),
                5,
            )
            .await
            .expect_err("non-owners must not update labels");
            assert_eq!(
                error,
                AppError::Forbidden("You do not have permission to update a label".into())
            );
        }
    }

    #[sqlx::test]
    async fn update_renames_the_label(pool: PgPool) {
        let label = create_test_record_label(&pool, 1).await.expect("label");
        assert_eq!(
            label.guest_plays, 5,
            "a fresh label starts on the column default"
        );
        let owner = create_test_user_with_permissions(&pool, 1, vec!["label_owner"])
            .await
            .expect("label_owner user");

        let updated = update_record_label(
            &pool,
            Some(&owner),
            label.id,
            "Renamed Label".into(),
            "new story".into(),
            "US ABC 1".into(),
            3,
        )
        .await
        .expect("label_owner update");

        assert_eq!(updated.name, "Renamed Label");
        assert_eq!(updated.description, "new story");
        assert_eq!(updated.isrc_base, "US ABC 1");
        assert_eq!(updated.guest_plays, 3, "the guest allowance saves too");
        // `update` re-slugifies from the name.
        assert_eq!(updated.slug, "renamed-label");
    }

    #[sqlx::test]
    async fn update_unknown_label_id_is_flattened(pool: PgPool) {
        let owner = create_test_user_with_permissions(&pool, 1, vec!["label_owner"])
            .await
            .expect("label_owner user");
        let error = update_record_label(
            &pool,
            Some(&owner),
            999_999,
            "x".into(),
            String::new(),
            String::new(),
            5,
        )
        .await
        .expect_err("unknown id must fail");
        assert_eq!(
            error,
            AppError::Internal("Could not retrieve label, try again later".into())
        );
    }
}
