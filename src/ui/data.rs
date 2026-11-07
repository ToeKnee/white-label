//! Dioxus-native data loading for the UI.
//!
//! Page components reach these through `super::use_page` (with `page_data`
//! adapting the result), which suspends the enclosing `SuspenseBoundary`
//! until the answer is in hand and carries it to the client for hydration.
//!
//! Any typed argument has to be declared in the route literal (`?slug`,
//! `?artist_slug&release_slug`): dioxus only puts declared params in the
//! URL, and a browser refuses a body on GET, so an undeclared argument
//! breaks every client build. During SSR dioxus runs the call in-process
//! with the same `Db`/`CurrentUser` extractors the no-JS form handlers in
//! `crate::server` use, so page content is rendered server-side and the
//! logged-in user's visibility rules apply there too. Client builds get
//! generated stubs that fetch the same GET endpoints over HTTP.
//!
//! These endpoints are plain GETs returning real HTTP statuses via
//! `HttpError`. Like the form handlers, they go through the
//! `crate::services` functions, so permission checks and hidden-content
//! rules live in one place.

use dioxus::prelude::*;

use crate::api_types::artist::ArtistResult;
use crate::api_types::links::LinksResult;
use crate::api_types::menu::AdminMenu;
use crate::api_types::page::PageResult;
use crate::api_types::record_label::{LabelArtistResult, LabelPageResult, LabelResult};
use crate::api_types::release::{ReleaseResult, ReleasesResult};
use crate::api_types::track::TrackResult;
#[cfg(feature = "server")]
use crate::error::AppError;
use crate::forms::links::LinksForm;
use crate::models::auth::User;
#[cfg(feature = "server")]
use crate::models::record_label::RecordLabel;
// Server-only: the service calls live in the server halves of the fns
// below (the fullstack macros cfg them out of client builds).
#[cfg(feature = "server")]
use crate::services;

/// Maps a service-layer [`AppError`] onto a dioxus `HttpError` with the
/// status from [`AppError::status`], keeping the message user-presentable.
/// The `HttpError` constructors return a ready `Result`, so a service fn
/// ends by matching the service result and handing its error here. Shared
/// with the upload server fn in [`super::upload`].
#[cfg(feature = "server")]
pub(crate) fn to_http_error<T>(error: &AppError) -> core::result::Result<T, HttpError> {
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
/// `label_owner` check the admin pages and the form handlers apply.
#[cfg(feature = "server")]
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
#[get("/api/get_artist?slug", db: crate::server::current_user::Db)]
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
#[get("/api/get_links?artist_slug", db: crate::server::current_user::Db)]
pub async fn get_links(artist_slug: String) -> core::result::Result<LinksResult, HttpError> {
    match services::links::get_links_service(&db.0, artist_slug).await {
        Ok(result) => Ok(result),
        Err(error) => to_http_error(&error),
    }
}

/// Save an artist's music-service and social links, returning the rows as
/// stored. The POST twin of the wire `UpdateLinks` endpoint, used by the
/// reactive links form (`ui::admin::links`) once the client build lands;
/// the no-JS fallback is the 303 handler at `POST /admin/forms/links`
/// (`crate::server::forms`). The service requires admin *and*
/// `label_owner`, so guests and owner-only users get a 403.
///
/// # Errors
/// 403 when the session lacks the permissions, 404 when the slug matches
/// no artist, 500 on database or row-handling failure.
#[post("/api/update_links",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser
)]
pub async fn update_links(form: LinksForm) -> core::result::Result<LinksResult, HttpError> {
    match services::links::update_links_service(&db.0, user.0.as_ref(), form).await {
        Ok(result) => Ok(result),
        Err(error) => to_http_error(&error),
    }
}

/// An artist's releases. `label_owner` sessions also see hidden releases.
///
/// # Errors
/// 404 when the slug matches no artist, 500 on database failure.
#[get("/api/get_releases?slug",
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
#[get("/api/get_release?artist_slug&release_slug",
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

/// One track within a release (the track editor's data; the track list
/// reuses [`get_release`], whose result already carries the tracks).
///
/// # Errors
/// 404 when any of the three slugs match nothing (the service checks
/// artist, release and track), 401/403 when the session is missing or
/// lacks `label_owner`, 500 on database failure.
#[get("/api/get_track?artist_slug&release_slug&track_slug",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser
)]
pub async fn get_track(
    artist_slug: String,
    release_slug: String,
    track_slug: String,
) -> core::result::Result<TrackResult, HttpError> {
    match services::track::get_track_service(
        &db.0,
        user.0.as_ref(),
        artist_slug,
        release_slug,
        track_slug,
    )
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
#[get("/api/get_page?slug",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser
)]
pub async fn get_page(slug: String) -> core::result::Result<PageResult, HttpError> {
    match services::page::get_page_service(&db.0, user.0.as_ref(), slug).await {
        Ok(result) => Ok(result),
        Err(error) => to_http_error(&error),
    }
}

/// The signed-in user for this request's session, or `None` for a guest —
/// the GET twin of the wire `POST /api/user` endpoint.
///
/// This is what `super::use_user` reads, so the navbar, the guards and the
/// profile pages all see the server's own session state.
#[expect(
    clippy::unused_async,
    reason = "the #[get] server fn's client-side stub is async; the signature stays async for symmetry with it"
)]
#[get("/api/user", user: crate::server::current_user::CurrentUser)]
pub async fn get_current_user() -> core::result::Result<Option<User>, HttpError> {
    Ok(user.0)
}

/// The admin side menu: label, artists with their releases, and pages,
/// each with its admin URL. `crate::services::menu` requires the `admin`
/// permission, so guests get a 401 the `AdminMenu` component answers with
/// its static skeleton.
///
/// # Errors
/// 401/403 when the session is not an admin, 500 with a retry message when
/// any part of the menu data fails to load.
#[get("/api/admin_menu",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser
)]
pub async fn get_admin_menu() -> core::result::Result<AdminMenu, HttpError> {
    match services::menu::admin_menu(&db.0, user.0.as_ref()).await {
        Ok(menu) => Ok(menu),
        Err(error) => to_http_error(&error),
    }
}

/// The label's pages by label id, for the dashboard's pages table.
/// Guests only see published, non-deleted pages; `label_owner` sessions
/// also see hidden ones.
///
/// # Errors
/// 500 with a retry message when the label or pages cannot be loaded.
#[get("/api/record_label_pages?record_label_id",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser
)]
pub async fn get_label_pages(
    record_label_id: i64,
) -> core::result::Result<LabelPageResult, HttpError> {
    let record_label = match RecordLabel::get_by_id(&db.0, record_label_id).await {
        Ok(record_label) => record_label,
        Err(error) => {
            tracing::error!("get_label_pages: Error while getting label: {error:?}");
            return HttpError::internal_server_error("Could not retrieve label, try again later");
        }
    };
    let pages = match record_label
        .pages(&db.0, show_hidden(user.0.as_ref()))
        .await
    {
        Ok(pages) => pages,
        Err(error) => {
            tracing::error!("get_label_pages: Error while getting pages: {error:?}");
            return HttpError::internal_server_error("Could not retrieve pages, try again later");
        }
    };
    Ok(LabelPageResult { pages })
}

/// The next scheduled release (a future `release_date`), if any — the
/// dashboard countdown's data. It takes no artist filter, because the
/// countdown never needs one.
///
/// # Errors
/// 500 with a retry message when the label or release cannot be loaded.
#[get("/api/get_next_scheduled_release", db: crate::server::current_user::Db)]
pub async fn get_next_scheduled_release() -> core::result::Result<Option<ReleaseResult>, HttpError>
{
    let record_label = match RecordLabel::first(&db.0).await {
        Ok(record_label) => record_label,
        Err(error) => {
            tracing::error!("get_next_scheduled_release: Error while getting label: {error:?}");
            return HttpError::internal_server_error("Failed to retrieve record label");
        }
    };
    match services::release::get_next_scheduled_release_service(&db.0, None, record_label.id).await
    {
        Ok(result) => Ok(result),
        Err(error) => to_http_error(&error),
    }
}
