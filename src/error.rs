//! Framework-neutral error type for the app's services layer.
//!
//! Services return `AppError`; the serving layer maps it to whatever the
//! stack at the boundary expects:
//!
//! - Live Leptos server fns (today): the conversion is automatic —
//!   `ServerFnError` implements `From<E: std::error::Error>` into its
//!   `ServerError(String)` variant, the same variant every service error
//!   used before this type existed, so the wire response is unchanged.
//! - Phase 2 Dioxus endpoints: build `HttpError`s from [`AppError::status`]
//!   plus the message, so these variants become real 400/401/403/404/500s.
//!
//! Example (Phase 2 handler shape):
//!
//! ```ignore
//! let artist = get_artist_service(&pool, slug).await?; // Err(AppError::NotFound(..))
//! ```

/// Error from a service call, with an HTTP status hint.
///
/// The wrapped string is user-presentable (the UI shows it directly), so it
/// must not contain connection strings, SQL, or secrets.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AppError {
    /// The request was malformed or a submitted form failed validation.
    #[error("{0}")]
    BadRequest(String),
    /// The caller is not (or no longer) authenticated.
    #[error("{0}")]
    Unauthorized(String),
    /// The caller is authenticated but lacks a required permission.
    #[error("{0}")]
    Forbidden(String),
    /// The addressed entity does not exist.
    #[error("{0}")]
    NotFound(String),
    /// Anything that went wrong server-side; details are in the tracing log.
    #[error("{0}")]
    Internal(String),
}

impl AppError {
    /// Wrap any displayable failure as [`AppError::Internal`].
    #[expect(
        clippy::needless_pass_by_value,
        reason = "call sites read `AppError::internal(err)`; the source is only stringified, never moved"
    )]
    pub fn internal(source: impl ToString) -> Self {
        Self::Internal(source.to_string())
    }

    /// The HTTP status this kind of failure suggests.
    #[must_use]
    pub const fn status(&self) -> u16 {
        match self {
            Self::BadRequest(_) => 400,
            Self::Unauthorized(_) => 401,
            Self::Forbidden(_) => 403,
            Self::NotFound(_) => 404,
            Self::Internal(_) => 500,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_is_the_bare_message() {
        let error = AppError::NotFound("Artist not found.".into());
        assert_eq!(error.to_string(), "Artist not found.");
    }

    #[test]
    fn internal_wraps_any_displayable_source() {
        let from_str = AppError::internal("boom");
        let wrapped: AppError = AppError::internal(std::io::Error::other("disk full"));
        assert_eq!(from_str, AppError::Internal("boom".into()));
        assert_eq!(wrapped, AppError::Internal("disk full".into()));
    }

    #[test]
    fn status_per_variant() {
        assert_eq!(AppError::BadRequest("x".into()).status(), 400);
        assert_eq!(AppError::Unauthorized("x".into()).status(), 401);
        assert_eq!(AppError::Forbidden("x".into()).status(), 403);
        assert_eq!(AppError::NotFound("x".into()).status(), 404);
        assert_eq!(AppError::Internal("x".into()).status(), 500);
    }
}
