//! The play-statistics pages' data: one scope (label, artist or release)
//! over one range, as graph points plus a track table.
//!
//! The pages sit behind the same `label_owner` permission as the rest of
//! the admin editors (see [`crate::services::menu`]'s gate), and their
//! scopes deliberately exclude soft-deleted rows — statistics describe the
//! live catalogue. How the counters accumulate (and which reports reach
//! them at all) lives in [`crate::services::track_play_stats`]; the shape
//! of the queries lives in
//! [`crate::models::track_play_stats::StatsScope`].

#[cfg(feature = "server")]
use crate::api_types::stats::StatsResult;
#[cfg(feature = "server")]
use crate::error::AppError;
#[cfg(feature = "server")]
use crate::models::artist::Artist;
#[cfg(feature = "server")]
use crate::models::auth::User;
#[cfg(feature = "server")]
use crate::models::record_label::RecordLabel;
#[cfg(feature = "server")]
use crate::models::release::Release;
#[cfg(feature = "server")]
use crate::models::track_play_stats::{PlaySort, StatsScope};
#[cfg(feature = "server")]
use crate::services::authentication_helpers::user_with_permissions;
#[cfg(feature = "server")]
use sqlx::PgPool;

/// The range codes a stats page's links carry, mapped to their first day.
/// Anything else (including an empty range, which the pages treat as
/// "all") is refused with a 400.
#[cfg(feature = "server")]
fn range_start(range: &str) -> Result<Option<chrono::NaiveDate>, AppError> {
    let today = chrono::Utc::now().date_naive();
    // A year cut as 364 days back keeps a 365-day window inclusive.
    let days = match range {
        "all" | "" => return Ok(None),
        "7d" => 6,
        "30d" => 29,
        "1y" => 364,
        other => {
            return Err(AppError::BadRequest(format!(
                "Unknown statistics range {other}."
            )));
        }
    };
    Ok(Some(today - chrono::Duration::days(days)))
}

/// The table's sort code, checked against [`PlaySort`] (whose variants carry
/// the `ORDER BY` the query uses). An unknown code is a 400 rather than a
/// quiet default, so a broken column link is visible instead of silently
/// answering a different order; an empty code is the default.
#[cfg(feature = "server")]
fn sort_key(sort: &str) -> Result<PlaySort, AppError> {
    PlaySort::from_code(sort)
        .ok_or_else(|| AppError::BadRequest(format!("Unknown statistics sort {sort}.")))
}

/// Resolve a scope's address (kind plus slug) to its row id, 404ing an
/// unknown slug. The label scope has no slug to speak of — it is the single
/// label — so its pages send a placeholder the lookup ignores.
#[cfg(feature = "server")]
async fn scope_id(pool: &PgPool, scope: &str, slug: &str) -> Result<StatsScope, AppError> {
    match scope {
        "label" => Ok(StatsScope::Label(
            RecordLabel::first(pool)
                .await
                .map_err(AppError::internal)?
                .id,
        )),
        "artist" => Ok(StatsScope::Artist(
            Artist::find_by_slug(pool, slug.to_string())
                .await
                .map_err(AppError::internal)?
                .ok_or_else(|| {
                    AppError::NotFound(format!("Could not find artist with slug {slug}."))
                })?
                .id,
        )),
        "release" => Ok(StatsScope::Release(
            Release::find_by_slug(pool, slug.to_string())
                .await
                .map_err(AppError::internal)?
                .ok_or_else(|| {
                    AppError::NotFound(format!("Could not find release with slug {slug}."))
                })?
                .id,
        )),
        other => Err(AppError::BadRequest(format!(
            "Unknown statistics scope {other}."
        ))),
    }
}

/// The play statistics for one scope and range: the graph's day series and
/// the table's track rows, in the order `sort` names.
///
/// `label_owner` sessions only — guests get a 401 and signed-in non-owners
/// a 403, like the other admin services.
///
/// # Errors
/// If the caller is missing or lacks the permission, return 401/403
/// If the scope kind, the range or the sort is unknown, return 400
/// If the scoped row (artist or release) does not exist, return 404
/// If the label is missing or the counters cannot be read, return 500
#[cfg(feature = "server")]
pub async fn play_stats_service(
    pool: &PgPool,
    user: Option<&User>,
    scope: &str,
    slug: &str,
    range: &str,
    sort: &str,
) -> Result<StatsResult, AppError> {
    user_with_permissions(user, vec!["label_owner"])?;
    let scope = scope_id(pool, scope, slug).await?;
    let since = range_start(range)?;
    let sort = sort_key(sort)?;

    let series = scope.day_series(pool, since).await.map_err(|e| {
        let err = format!("Error while reading the play statistics graph: {e:?}");
        tracing::error!("{err}");
        AppError::internal(e)
    })?;
    let rows = scope.track_totals(pool, since, sort).await.map_err(|e| {
        let err = format!("Error while reading the play statistics table: {e:?}");
        tracing::error!("{err}");
        AppError::internal(e)
    })?;
    Ok(StatsResult { series, rows })
}
