//! This module contains the service layers for the application. All actions should go through these services to ensure proper separation of concerns and maintainability.

#[cfg(feature = "server")]
use crate::models::Visibility;
#[cfg(feature = "server")]
use crate::models::auth::User;

/// Which rung of [`Visibility`] a list load sits at for a session.
///
/// Guests see `Published` rows only. A `label_owner` session also sees
/// unpublished ones (`Hidden`); the admin "show deleted" toggle
/// (`include_deleted`, carried by the admin listing pages) raises that to
/// `Deleted`. The same permission test the `label_owner` checks elsewhere
/// apply.
#[cfg(feature = "server")]
#[must_use]
pub fn visibility_for(current_user: Option<&User>, include_deleted: bool) -> Visibility {
    let label_owner = current_user.is_some_and(|user| user.permissions.contains("label_owner"));
    match (label_owner, include_deleted) {
        (false, _) => Visibility::Published,
        (true, false) => Visibility::Hidden,
        (true, true) => Visibility::Deleted,
    }
}

pub mod artist;
pub mod artists;
pub mod authentication_helpers;
pub mod files;
pub mod links;
pub mod menu;
pub mod page;
pub mod record_label;
pub mod release;
pub mod track;
pub mod user;
