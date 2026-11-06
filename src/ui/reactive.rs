//! Shared plumbing for the reactive editors (MIGRATION.md Phase 6 slice 6i).
//!
//! Every admin editor has two submission paths that must behave the same: the
//! javascript-off `<form action="/admin/forms/…">` post (whose 303 the page
//! reads back as `?updated=1` / `?bad=<code>`) and the reactive one, where a
//! client build intercepts the submit and calls a JSON twin (`/api/client/…`)
//! through `use_action`. A webview window cannot follow a 303 at all, so the
//! reactive path is the only one that works there.
//!
//! This module holds the two pieces every editor needs:
//!
//! - reading a submit's inputs into the posted-field structs in
//!   [`crate::forms`] (Dioxus' `parsed_values` deserializer sits behind a
//!   dioxus-html feature the dioxus meta crate does not expose, so the values
//!   are mapped to JSON by hand);
//! - turning an action's answer ([`crate::api_types::edit::EditOutcome`]) into
//!   what the page shows: the saved row, the fixed refusal code, or the
//!   transport failure.
//!
//! ```ignore
//! let mut edit = use_action(move |request: ReleaseEdit| async move {
//!     apply_release_edit(request).await
//! });
//! // The row the page renders follows the last save (a renamed release carries
//! // its new slug, a deleted one `deleted_at`).
//! let release = saved_row(edit.value()).unwrap_or_else(|| fetched_release.clone());
//! let message = refused_code(edit.value())
//!     .and_then(|code| release_message(Some(code.as_str())).map(str::to_string))
//!     .or_else(|| transport_message(edit.value()));
//! ```

use dioxus::CapturedError;
use dioxus::prelude::*;
use serde::de::DeserializeOwned;

use crate::api_types::edit::EditOutcome;

/// What an action's `value()` hands back: the answer, or the error dioxus
/// captured while running it (a transport failure — unreachable API, 500).
type Answer<T> = Option<Result<ReadSignal<T>, CapturedError>>;

/// The submit's text inputs as a JSON object, keyed by input `name`.
///
/// File inputs are skipped: uploads have their own reactive path
/// ([`crate::ui::upload::FileUploadWithProgress`]).
fn form_json(evt: &FormEvent) -> serde_json::Map<String, serde_json::Value> {
    let mut fields = serde_json::Map::new();
    for (name, value) in evt.values() {
        if let FormValue::Text(text) = value {
            fields.insert(name, serde_json::Value::String(text));
        }
    }
    fields
}

/// [`form_json`], with a repeated checkbox group joined.
///
/// "All Artists" (`artist_ids`) posts once per checked box; this joins those
/// values into the comma-separated string the service forms take — which is
/// what [`crate::server::forms`] does for the javascript-off post, so both
/// paths hand the services the same shape.
///
/// Boxes that post an empty value are dropped: an artist id is never empty,
/// and depending on the client an unchecked box arrives either not at all or
/// with an empty value.
fn form_json_joined(
    evt: &FormEvent,
    repeated_key: &str,
) -> serde_json::Map<String, serde_json::Value> {
    let mut fields = serde_json::Map::new();
    let mut ids = Vec::new();
    for (name, value) in evt.values() {
        let FormValue::Text(text) = value else {
            continue;
        };
        if name == repeated_key {
            if !text.is_empty() {
                ids.push(text);
            }
        } else {
            fields.insert(name, serde_json::Value::String(text));
        }
    }
    fields.insert(
        repeated_key.to_string(),
        serde_json::Value::String(ids.join(",")),
    );
    fields
}

/// The posted-field struct ([`crate::forms`]) for this submit.
///
/// An unparsable body becomes the struct's `Default`, which routes a
/// hand-crafted post through the service's own validation (the
/// `name_required`/`no_label` alerts) rather than inventing a value — the same
/// choice the javascript-off handlers make.
pub(crate) fn posted_fields<T: DeserializeOwned + Default>(evt: &FormEvent) -> T {
    serde_json::from_value(serde_json::Value::Object(form_json(evt))).unwrap_or_default()
}

/// [`posted_fields`] for a form carrying the repeated artist-checkbox group
/// (`artist_ids`).
pub(crate) fn posted_fields_with_ids<T: DeserializeOwned + Default>(
    evt: &FormEvent,
    repeated_key: &str,
) -> T {
    serde_json::from_value(serde_json::Value::Object(form_json_joined(
        evt,
        repeated_key,
    )))
    .unwrap_or_default()
}

/// The row the last editor action saved.
///
/// `None` if it has not run or did not save. Render the page from this when it
/// is set, so a rename, a soft-delete or a restore is reflected without a page
/// load.
pub(crate) fn saved_row<T: Clone + 'static>(answer: Answer<EditOutcome<T>>) -> Option<T> {
    match answer?.ok().map(|signal| signal.cloned())? {
        EditOutcome::Saved(row) => Some(row),
        EditOutcome::Failed { .. } => None,
    }
}

/// The fixed code the last editor action was refused with.
///
/// `None` if it has not run, saved, or failed at the request level instead.
/// Each editor maps this code through the same table its `?bad=` path uses.
pub(crate) fn refused_code<T: Clone + 'static>(answer: Answer<EditOutcome<T>>) -> Option<String> {
    match answer?.ok().map(|signal| signal.cloned())? {
        EditOutcome::Saved(_) => None,
        EditOutcome::Failed { code } => Some(code),
    }
}

/// A transport failure from an action.
///
/// The API was unreachable, or a twin answered 500 (a session store that will
/// not flush, for instance). The message is always one of ours — a refusal by
/// code arrives as data, so the database's own text never reaches the page.
pub(crate) fn transport_message<T: 'static>(answer: Answer<T>) -> Option<String> {
    answer.and_then(|result| result.err().map(|error| error.to_string()))
}
