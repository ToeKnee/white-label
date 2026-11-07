//! The row-visibility ladder shared by the model list queries.

/// What a list query may include. The rungs are cumulative: each one adds
/// rows to the one above.
///
/// ```text
/// Published  — published and not deleted (what a guest sees)
/// Hidden     — adds unpublished rows           (staff default)
/// Deleted    — adds soft-deleted rows          (admin "show deleted")
/// ```
///
/// Single-row lookups (`find_by_slug` and friends) keep their
/// `include_hidden: bool`: opening one known row (for example an admin
/// editing a row they just deleted, to restore it) is not a listing
/// decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Visibility {
    /// Published, non-deleted rows only.
    Published,
    /// Also unpublished rows (a future or empty `published_at`).
    Hidden,
    /// Also soft-deleted rows (`deleted_at` set).
    Deleted,
}
