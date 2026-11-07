//! The shared outcome type for the reactive editors.

/// What a reactive editor action answered.
///
/// A javascript-off save reports itself with a redirect carrying `?updated=1`
/// or `?bad=<code>`; a client build has no page load to carry that (and a
/// webview window cannot follow a 303 at all), so the reactive twins answer
/// with the same information as data:
///
/// ```text
/// POST /api/update_release  {"fields":{…}}
/// 200 {"Saved":{"release":{…},"artists":[…],"tracks":[…]}}
///
/// POST /api/update_release  {"fields":{…}}
/// 200 {"Failed":{"code":"slug_taken"}}
/// ```
///
/// `T` is the page's own result type (`ReleaseResult`, `ArtistResult`, …), or
/// the row a non-admin editor stores (`User` for the profile page), so an
/// editor can re-render straight from the save: the success alert, the status
/// badge and the Delete/Restore button all follow it instead of waiting for a
/// reload. Refusals carry the very same fixed codes the 303 handlers put in
/// `?bad=`, which each editor already maps to alert text. A transport
/// failure (unreachable API, session store down) stays a request-level error.
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub enum EditOutcome<T> {
    /// The change was applied; the payload is the row as the server now holds
    /// it (a renamed row carries its new slug, a deleted one `deleted_at`).
    Saved(T),
    /// The change was refused.
    Failed {
        /// The fixed code, never a message from the database.
        code: String,
    },
}
