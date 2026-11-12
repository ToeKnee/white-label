//! The counting rules behind the global player's play reports (see
//! [`crate::ui::player`]'s statistics reporting): one report per play the
//! bar stops, ending as complete, partial or skip.

#[cfg(feature = "server")]
use crate::api_types::play_stats::{PlayCounted, PlayReport};
#[cfg(feature = "server")]
use crate::error::AppError;
#[cfg(feature = "server")]
use crate::models::auth::User;
#[cfg(feature = "server")]
use crate::models::track::Track;
#[cfg(feature = "server")]
use crate::models::track_play_stats::TrackPlayStats;
#[cfg(feature = "server")]
use sqlx::PgPool;

/// Count one reported play against its track's counters.
///
/// A `label_owner` session's plays are not counted: label owners preview
/// their own catalogue constantly, and the counters are meant to measure
/// the audience. Their reports still answer Ok with `counted: false` (not
/// an error), so a client reporting a preview learns it was dropped without
/// treating it as a failure. Everyone else — guests included — is counted.
///
/// The track is looked up by slug alone: the player can only have loaded a
/// row its page could show, and an unknown slug is a 404 rather than a
/// silently dropped report, so broken clients stay visible in the logs.
///
/// # Errors
/// If the track cannot be found, return 404
/// If the track cannot be looked up or the counters cannot be written,
/// return 500
#[cfg(feature = "server")]
pub async fn report_play_service(
    pool: &PgPool,
    user: Option<&User>,
    report: PlayReport,
) -> Result<PlayCounted, AppError> {
    if user.is_some_and(|user| user.permissions.contains("label_owner")) {
        return Ok(PlayCounted { counted: false });
    }

    let track = Track::find_by_slug(pool, report.track_slug.clone())
        .await
        .map_err(|e| {
            let err = format!("Error while finding the played track: {e:?}");
            tracing::error!("{err}");
            AppError::internal(e)
        })?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Could not find track with slug {}.",
                report.track_slug
            ))
        })?;

    TrackPlayStats::record_play(pool, track.id, report.kind)
        .await
        .map_err(|e| {
            let err = format!(
                "Error while counting the play of track {}: {e:?}",
                report.track_slug
            );
            tracing::error!("{err}");
            AppError::internal(e)
        })?;

    Ok(PlayCounted { counted: true })
}
