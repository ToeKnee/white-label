//! Dioxus-native data loading for the UI (MIGRATION.md Phase 4 slice 2).
//!
//! Each page component calls one of these from `use_resource`, and reads
//! the value inside a `SuspenseBoundary` via `Resource::suspend()`. During
//! SSR dioxus runs the call in-process with the same axum extractors the
//! Phase 2 `/api/*` port uses (`Db`, `CurrentUser`), so page content is
//! rendered server-side and the logged-in user's visibility rules apply
//! there too (extractor injection in SSR was verified in the Phase 0
//! spike). Client builds get generated stubs that fetch the same GET
//! endpoints over HTTP once the client targets land (Phases 6-7).
//!
//! These endpoints deliberately sit beside the Phase 2 port, not on top of
//! it: the port keeps its exact Leptos wire protocol (POST,
//! `"Variant|message"` errors) for the Leptos site until cutover, while
//! these are plain GETs returning real HTTP statuses via `HttpError`.
//! Both go through the same `crate::services` functions, so permission
//! checks and hidden-content rules live in one place.

use dioxus::prelude::*;

use crate::error::AppError;
use crate::models::auth::User;
use crate::models::record_label::RecordLabel;
use crate::routes::artist::ArtistResult;
use crate::routes::links::LinksResult;
use crate::routes::page::PageResult;
use crate::routes::record_label::{LabelArtistResult, LabelResult};
use crate::routes::release::{ReleaseResult, ReleasesResult};
use crate::services;

/// Maps a service-layer [`AppError`] onto a dioxus `HttpError` with the
/// status from [`AppError::status`], keeping the message user-presentable.
/// The `HttpError` constructors return a ready `Result<T, HttpError>`, so
/// service fns end with `match result { Ok(value) => Ok(value), Err(e) =>
/// to_http_error(&e) }`.
fn to_http_error<T>(error: &AppError) -> core::result::Result<T, HttpError> {
    let message = error.to_string();
    match error.status() {
        400 => HttpError::bad_request(message),
        401 => HttpError::unauthorized(message),
        403 => HttpError::forbidden(message),
        404 => HttpError::not_found(message),
        _ => HttpError::internal_server_error(message),
    }
}

/// Whether hidden (unpublished or deleted) content is listed — the same
/// `label_owner` check the Phase 2 handlers and Leptos routes use.
fn show_hidden(current_user: Option<&User>) -> bool {
    current_user.is_some_and(|user| user.permissions.contains("label_owner"))
}

/// The first record label, which drives the navbar brand, the page title
/// and the home page heading.
///
/// # Errors
/// 500 with a retry message when the label cannot be loaded.
#[get("/api/record_label", db: crate::server::current_user::Db)]
pub async fn get_record_label() -> core::result::Result<LabelResult, HttpError> {
    let record_label = match RecordLabel::first(&db.0).await {
        Ok(record_label) => record_label,
        Err(error) => {
            tracing::error!("get_record_label: Error while getting record label: {error:?}");
            return HttpError::internal_server_error(
                "Could not retrieve record label, try again later",
            );
        }
    };
    Ok(LabelResult { record_label })
}

/// The label's artists. Guests only see published, non-deleted artists;
/// `label_owner` sessions also see hidden ones.
///
/// # Errors
/// 500 with a retry message when the label or artists cannot be loaded.
#[get("/api/record_label_artists",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser
)]
pub async fn get_label_artists() -> core::result::Result<LabelArtistResult, HttpError> {
    let record_label = match RecordLabel::first(&db.0).await {
        Ok(record_label) => record_label,
        Err(error) => {
            tracing::error!("get_label_artists: Error while getting label: {error:?}");
            return HttpError::internal_server_error("Could not retrieve label, try again later");
        }
    };
    let artists = match record_label
        .artists(&db.0, show_hidden(user.0.as_ref()))
        .await
    {
        Ok(artists) => artists,
        Err(error) => {
            tracing::error!("get_label_artists: Error while getting artists: {error:?}");
            return HttpError::internal_server_error("Could not retrieve artists, try again later");
        }
    };
    Ok(LabelArtistResult { artists })
}

/// One artist by slug (public page data).
///
/// # Errors
/// 404 when the slug matches no artist, 500 on database failure.
#[get("/api/get_artist", db: crate::server::current_user::Db)]
pub async fn get_artist(slug: String) -> core::result::Result<ArtistResult, HttpError> {
    match services::artist::get_artist_service(&db.0, slug).await {
        Ok(result) => Ok(result),
        Err(error) => to_http_error(&error),
    }
}

/// An artist's music-service and social links.
///
/// # Errors
/// 404 when the slug matches no artist, 500 on database failure.
#[get("/api/get_links", db: crate::server::current_user::Db)]
pub async fn get_links(artist_slug: String) -> core::result::Result<LinksResult, HttpError> {
    match services::links::get_links_service(&db.0, artist_slug).await {
        Ok(result) => Ok(result),
        Err(error) => to_http_error(&error),
    }
}

/// An artist's releases. `label_owner` sessions also see hidden releases.
///
/// # Errors
/// 404 when the slug matches no artist, 500 on database failure.
#[get("/api/get_releases",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser
)]
pub async fn get_releases(slug: String) -> core::result::Result<ReleasesResult, HttpError> {
    match services::release::get_releases_service(&db.0, user.0.as_ref(), slug).await {
        Ok(result) => Ok(result),
        Err(error) => to_http_error(&error),
    }
}

/// One release through its artist (the public release page data).
///
/// # Errors
/// 404 when the release is not visible to this viewer, 500 on database
/// failure.
#[get("/api/get_release",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser
)]
pub async fn get_release(
    artist_slug: String,
    release_slug: String,
) -> core::result::Result<ReleaseResult, HttpError> {
    match services::release::get_release_service(&db.0, user.0.as_ref(), artist_slug, release_slug)
        .await
    {
        Ok(result) => Ok(result),
        Err(error) => to_http_error(&error),
    }
}

/// One marketing/legal page by slug. `label_owner` sessions can also see
/// unpublished pages.
///
/// # Errors
/// 404 when the slug matches no visible page, 500 on database failure.
#[get("/api/get_page",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser
)]
pub async fn get_page(slug: String) -> core::result::Result<PageResult, HttpError> {
    match services::page::get_page_service(&db.0, user.0.as_ref(), slug).await {
        Ok(result) => Ok(result),
        Err(error) => to_http_error(&error),
    }
}
