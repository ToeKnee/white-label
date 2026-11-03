//! Record-label endpoints ported from `routes/record_label.rs` (see the
//! `api` module docs for the wire protocol). The reads keep their inline
//! model calls (flattened error messages matching the Leptos code byte for
//! byte); the update runs through `services::record_label`, shared with the
//! Dioxus `EditLabel` form handler in `server::forms`.

use axum::response::Response;

use crate::models::record_label::RecordLabel;
use crate::routes::record_label::{
    GetLabelArtists, GetLabelPages, GetRecordLabel, LabelArtistResult, LabelPageResult,
    LabelResult, UpdateRecordLabel,
};
use crate::server::api::{WireError, decode_args, json_response};
use crate::server::current_user::{CurrentUser, Db};
use crate::services;

const PATH_RECORD_LABEL: &str = "/api/record_label";
const PATH_RECORD_LABEL_ARTISTS: &str = "/api/record_label_artists";
const PATH_RECORD_LABEL_PAGES: &str = "/api/record_label_pages";
const PATH_UPDATE_RECORD_LABEL: &str = "/api/update_record_label";

/// Whether hidden (unpublished/deleted) content should be listed, mirroring
/// the routes' `permissions.contains("label_owner")` check.
fn show_hidden(current_user: Option<&crate::models::auth::User>) -> bool {
    current_user.is_some_and(|user| user.permissions.contains("label_owner"))
}

/// Get the first record label. Public.
pub async fn get_record_label(Db(pool): Db, body: String) -> Result<Response, WireError> {
    decode_args::<GetRecordLabel>(PATH_RECORD_LABEL, &body)?;

    let record_label = RecordLabel::first(&pool).await.map_err(|error| {
        tracing::error!("get_record_label: {error:?}");
        WireError::internal(
            PATH_RECORD_LABEL,
            "Could not retrieve record label, try again later",
        )
    })?;

    json_response(PATH_RECORD_LABEL, &LabelResult { record_label })
}

/// List the label's artists. Public — `label_owner` sessions also see
/// unpublished and deleted artists.
pub async fn get_label_artists(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    decode_args::<GetLabelArtists>(PATH_RECORD_LABEL_ARTISTS, &body)?;

    let record_label = RecordLabel::first(&pool).await.map_err(|error| {
        tracing::error!("get_label_artists: {error:?}");
        WireError::internal(
            PATH_RECORD_LABEL_ARTISTS,
            "Could not retrieve label, try again later",
        )
    })?;
    let artists = record_label
        .artists(&pool, show_hidden(current_user.as_ref()))
        .await
        .map_err(|error| {
            tracing::error!("get_label_artists: {error:?}");
            WireError::internal(
                PATH_RECORD_LABEL_ARTISTS,
                "Could not retrieve artists, try again later",
            )
        })?;

    json_response(PATH_RECORD_LABEL_ARTISTS, &LabelArtistResult { artists })
}

/// List one label's pages by id. Public — `label_owner` sessions also see
/// unpublished and deleted pages.
pub async fn get_label_pages(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<GetLabelPages>(PATH_RECORD_LABEL_PAGES, &body)?;

    let record_label = RecordLabel::get_by_id(&pool, args.record_label_id)
        .await
        .map_err(|error| {
            tracing::error!("get_label_pages: {error:?}");
            WireError::internal(
                PATH_RECORD_LABEL_PAGES,
                "Could not retrieve label, try again later",
            )
        })?;
    let pages = record_label
        .pages(&pool, show_hidden(current_user.as_ref()))
        .await
        .map_err(|error| {
            tracing::error!("get_label_pages: {error:?}");
            WireError::internal(
                PATH_RECORD_LABEL_PAGES,
                "Could not retrieve pages, try again later",
            )
        })?;

    json_response(PATH_RECORD_LABEL_PAGES, &LabelPageResult { pages })
}

/// Update the label's name, description and ISRC base (`label_owner` only).
/// The checks and messages live in [`services::record_label`] (shared with
/// the `forms` `EditLabel` handler); `from_app_error` keeps the statuses and
/// `ServerError|<message>` bodies the Leptos client expects.
pub async fn update_record_label(
    Db(pool): Db,
    CurrentUser(current_user): CurrentUser,
    body: String,
) -> Result<Response, WireError> {
    let args = decode_args::<UpdateRecordLabel>(PATH_UPDATE_RECORD_LABEL, &body)?;

    let record_label = services::record_label::update_record_label(
        &pool,
        current_user.as_ref(),
        args.id,
        args.name,
        args.description,
        args.isrc_base,
    )
    .await
    .map_err(|error| WireError::from_app_error(PATH_UPDATE_RECORD_LABEL, &error))?;

    json_response(PATH_UPDATE_RECORD_LABEL, &LabelResult { record_label })
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::{
        PATH_RECORD_LABEL, PATH_RECORD_LABEL_ARTISTS, PATH_RECORD_LABEL_PAGES,
        PATH_UPDATE_RECORD_LABEL,
    };
    use crate::models::test_helpers::{
        create_test_artist, create_test_page, create_test_record_label,
        create_test_user_with_permissions,
    };
    use crate::routes::auth::Login;
    use crate::routes::record_label::{
        GetLabelArtists, GetLabelPages, GetRecordLabel, LabelArtistResult, LabelPageResult,
        LabelResult, UpdateRecordLabel,
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
    async fn record_label_returns_the_first_label(pool: PgPool) {
        create_test_record_label(&pool, 1).await.expect("label");
        let server = api_server(pool).await;

        let response = post_args(&server, PATH_RECORD_LABEL, &GetRecordLabel {}).await;

        response.assert_status_ok();
        let result: LabelResult = response.json();
        assert_eq!(result.record_label.name, "Test Record Label 1");
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn record_label_with_no_label_is_flattened_internal(pool: PgPool) {
        // These endpoints flatten every failure into a generic message —
        // kept byte-identical to the Leptos routes.
        let server = api_server(pool).await;

        let response = post_args(&server, PATH_RECORD_LABEL, &GetRecordLabel {}).await;

        response.assert_status_internal_server_error();
        assert_eq!(
            response.text(),
            "ServerError|Could not retrieve record label, try again later"
        );
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn record_label_artists_hide_unpublished_from_anonymous(pool: PgPool) {
        let label = create_test_record_label(&pool, 2).await.expect("label");
        create_test_artist(&pool, 1, Some(label.clone()))
            .await
            .expect("published artist");
        let mut hidden = create_test_artist(&pool, 2, Some(label.clone()))
            .await
            .expect("artist to hide");
        hidden.published_at = None; // unpublished → hidden from anonymous
        hidden.update(&pool).await.expect("hide artist");

        let anonymous = api_server(pool.clone()).await;
        let public = post_args(&anonymous, PATH_RECORD_LABEL_ARTISTS, &GetLabelArtists {}).await;
        public.assert_status_ok();
        let public: LabelArtistResult = public.json();
        assert_eq!(public.artists.len(), 1);

        create_test_user_with_permissions(&pool, 2, vec!["label_owner"])
            .await
            .expect("label_owner user");
        let owner = api_server(pool).await;
        post_args(&owner, "/api/login", &login_args("test-2"))
            .await
            .assert_status_ok();
        let with_hidden = post_args(&owner, PATH_RECORD_LABEL_ARTISTS, &GetLabelArtists {}).await;
        with_hidden.assert_status_ok();
        let with_hidden: LabelArtistResult = with_hidden.json();
        assert_eq!(with_hidden.artists.len(), 2);
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn record_label_pages_returns_the_label_pages(pool: PgPool) {
        let label = create_test_record_label(&pool, 3).await.expect("label");
        create_test_page(&pool, 3, Some(label.clone()))
            .await
            .expect("test page");
        let server = api_server(pool).await;

        let response = post_args(
            &server,
            PATH_RECORD_LABEL_PAGES,
            &GetLabelPages {
                record_label_id: label.id,
            },
        )
        .await;

        response.assert_status_ok();
        let result: LabelPageResult = response.json();
        assert_eq!(result.pages.len(), 1);
        assert_eq!(result.pages[0].slug, "test-page-3");

        // Unknown label ids use the flattened message.
        let missing = post_args(
            &server,
            PATH_RECORD_LABEL_PAGES,
            &GetLabelPages {
                record_label_id: 999_999,
            },
        )
        .await;
        missing.assert_status_internal_server_error();
        assert_eq!(
            missing.text(),
            "ServerError|Could not retrieve label, try again later"
        );
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn update_record_label_without_login_is_unauthorized(pool: PgPool) {
        // Status upgraded from the Leptos 500; message text preserved.
        let server = api_server(pool).await;

        let response = post_args(
            &server,
            PATH_UPDATE_RECORD_LABEL,
            &UpdateRecordLabel {
                id: 1,
                name: "Hijacked Label".to_string(),
                description: String::new(),
                isrc_base: String::new(),
            },
        )
        .await;

        response.assert_status_unauthorized();
        assert_eq!(
            response.text(),
            "ServerError|You must be logged in to update a label"
        );
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn update_record_label_without_label_owner_is_forbidden(pool: PgPool) {
        create_test_record_label(&pool, 4).await.expect("label");
        // This route only checks `label_owner`, so a plain admin fails.
        create_test_user_with_permissions(&pool, 4, vec!["admin"])
            .await
            .expect("admin-only user");
        let server = api_server(pool).await;

        post_args(&server, "/api/login", &login_args("test-4"))
            .await
            .assert_status_ok();

        let response = post_args(
            &server,
            PATH_UPDATE_RECORD_LABEL,
            &UpdateRecordLabel {
                id: 1,
                name: "Hijacked Label".to_string(),
                description: String::new(),
                isrc_base: String::new(),
            },
        )
        .await;

        response.assert_status_forbidden();
        assert_eq!(
            response.text(),
            "ServerError|You do not have permission to update a label"
        );
    }

    #[expect(
        clippy::future_not_send,
        reason = "axum-test awaits a !Send auto-future; `#[sqlx::test]` runs this body via block_on on one thread, so Send isn't needed"
    )]
    #[sqlx::test]
    async fn label_owner_can_update_record_label(pool: PgPool) {
        let label = create_test_record_label(&pool, 5).await.expect("label");
        create_test_user_with_permissions(&pool, 5, vec!["label_owner"])
            .await
            .expect("label_owner user");
        let server = api_server(pool).await;

        post_args(&server, "/api/login", &login_args("test-5"))
            .await
            .assert_status_ok();

        let response = post_args(
            &server,
            PATH_UPDATE_RECORD_LABEL,
            &UpdateRecordLabel {
                id: label.id,
                name: "Renamed Label".to_string(),
                description: "new story".to_string(),
                isrc_base: "US ABC 1".to_string(),
            },
        )
        .await;

        response.assert_status_ok();
        let result: LabelResult = response.json();
        assert_eq!(result.record_label.name, "Renamed Label");
        assert_eq!(result.record_label.description, "new story");
        assert_eq!(result.record_label.isrc_base, "US ABC 1");
    }
}
