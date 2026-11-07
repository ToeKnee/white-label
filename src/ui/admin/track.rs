//! The track list and editors.
//!
//! Same shape as the release editors (`super::release`): pages post to
//! `crate::server::forms` handlers under `/admin/forms/track`, the handlers
//! 303 with fixed query codes (`?updated=1`, `?bad=<code>`), and
//! `track_message` maps the codes to text mirroring the service/model
//! validation messages. The page gate is `label_owner`; the track services
//! additionally require `admin`.
//!
//! The editors also save **reactively**: with a client
//! build `onsubmit` prevents the post and one `use_action` calls the JSON
//! twins at the bottom of this module (`/api/*_track`), which answer
//! the page's own `TrackResult` (or the same fixed codes as data). The row on
//! screen then follows the save — a rename carries the new slug into the next
//! save, and a soft-delete swaps Delete for Restore — with no page load,
//! which is what makes these editors usable in a desktop window (a 303 has
//! nothing to redirect there). A reactive create answers the new row and the
//! create page navigates to its editor. The plain form posts stay underneath
//! as the javascript-off path.
//!
//! Design notes:
//!
//! - The track list (a `TrackList`) renders from the release editor's
//!   existing `get_release` result (whose `tracks` field already carries
//!   them); the `/tracks` page fetches that same endpoint, and a dedicated
//!   `get_track` fetch serves the track editor.
//! - Track images show in the list (`primary_image_url`) but the editors
//!   offer no way to upload one.
//! - A reactive rename leaves the *URL* on the old slug (the javascript-off
//!   path 303s to the new one). Saving again, deleting and restoring all
//!   follow the new slug, but reloading the old URL gets the tracks list.

use dioxus::prelude::*;
use dioxus::router::RouterContext;

use crate::api_types::edit::EditOutcome;
use crate::api_types::track::TrackResult;
use crate::forms::track::{TrackCreateFields, TrackSlugFields, TrackUpdateFields};
use crate::models::track::Track;
use crate::ui::assets::display_image_url;
use crate::ui::data::{get_artist, get_release, get_track};
use crate::ui::page_data;
use crate::ui::reactive::{
    posted_fields, posted_fields_with_ids, refused_code, saved_row, transport_message,
};
use crate::ui::use_page;
use crate::ui::use_user;
use crate::ui::utils::{InlineError, Loading, Success, permission_allows};
use crate::ui::{Route, redirect};

// Server-only: the twins' bodies run in the server half of the `#[post]`
// macros, next to the same services the javascript-off handlers call.
#[cfg(feature = "server")]
use crate::server::forms::track_code;
#[cfg(feature = "server")]
use crate::services;

use super::pages::{DateField, MarkdownField};
use super::release::ArtistSelect;

/// The track list page for one release (the `/tracks` route).
#[component]
pub fn Tracks(
    /// The artist slug from the URL.
    artist_slug: String,
    /// The release slug from the URL.
    release_slug: String,
) -> Element {
    rsx! {
        SuspenseBoundary {
            fallback: |_context: SuspenseContext| rsx! { Loading {} },
            TracksBody { artist_slug, release_slug }
        }
    }
}

#[component]
fn TracksBody(artist_slug: String, release_slug: String) -> Element {
    let session = use_user()?;
    let user = session().ok().flatten();
    if !permission_allows(user.as_ref(), "label_owner") {
        redirect(Route::Dashboard {});
        return rsx! { Loading {} };
    }

    // One fetch serves both the existence checks and the rows (the result's
    // `tracks` field); a bad artist/release slug steers to the artist
    // profile, as the original's release-fetch failure did.
    let tracks_artist_slug = artist_slug.clone();
    let tracks_release_slug = release_slug.clone();
    let tracks_release_resource = use_page(move || {
        page_data(get_release(
            tracks_artist_slug.clone(),
            tracks_release_slug.clone(),
        ))
    })?;
    let tracks = match tracks_release_resource() {
        Ok(result) => result.tracks.iter().map(|twa| twa.track.clone()).collect(),
        Err(error) => {
            tracing::error!(
                "Error while getting release for the tracks list (artist {artist_slug}, release {release_slug}): {error}"
            );
            redirect(Route::EditArtist {
                artist_slug,
                updated: None,
                bad: None,
            });
            return rsx! { Loading {} };
        }
    };

    rsx! {
        TrackList { artist_slug, release_slug, tracks }
    }
}

/// The tracks table with its "Add" link, shared by the `/tracks` page and
/// the release editor's sidebar.
#[component]
pub fn TrackList(
    /// The artist the rows link back through.
    artist_slug: String,
    /// The release the rows link back through.
    release_slug: String,
    /// The tracks to list.
    tracks: Vec<Track>,
) -> Element {
    let add_route = Route::CreateTrack {
        artist_slug: artist_slug.clone(),
        release_slug: release_slug.clone(),
        bad: None,
    };

    rsx! {
        h2 { "Tracks" }
        div { class: "divider" }
        div { class: "overflow-x-auto",
            table { class: "table",
                thead {
                    tr {
                        th { "Name" }
                        th {}
                    }
                }
                tbody {
                    if tracks.is_empty() {
                        tr { td { colspan: 2, "No tracks found." } }
                    } else {
                        for track in tracks {
                            TrackRow {
                                track,
                                artist_slug: artist_slug.clone(),
                                release_slug: release_slug.clone(),
                            }
                        }
                    }
                    tr {
                        td {}
                        td {
                            Link { to: add_route, class: "btn btn-primary", "Add" }
                        }
                    }
                }
                tfoot {
                    tr {
                        th { "Name" }
                        th {}
                    }
                }
            }
        }
    }
}

/// One track row.
#[component]
fn TrackRow(track: Track, artist_slug: String, release_slug: String) -> Element {
    let isrc = track.isrc_code.clone().unwrap_or_default();
    let edit_route = Route::EditTrack {
        artist_slug,
        release_slug,
        track_slug: track.slug.clone(),
        updated: None,
        bad: None,
    };

    rsx! {
        tr {
            td {
                div { class: "flex gap-3 items-center",
                    div { class: "avatar",
                        div { class: "w-12 rounded-full not-prose",
                            img {
                                class: "m-0",
                                src: display_image_url(&track.primary_image_url()),
                                alt: track.name,
                            }
                        }
                    }
                    div {
                        div { class: "font-bold",
                            Link { to: edit_route.clone(), "{track.name}" }
                        }
                        div { class: "text-sm opacity-50",
                            "{isrc}"
                            br {}
                            " {track.slug}"
                        }
                    }
                }
            }
            td {
                Link { to: edit_route, class: "btn btn-primary", "Edit" }
            }
        }
    }
}

/// Create a track on a release; `bad` carries the create handler's code.
#[component]
pub fn CreateTrack(
    /// The artist slug from the URL.
    artist_slug: String,
    /// The release slug from the URL.
    release_slug: String,
    /// Error code from a failed create.
    bad: Option<String>,
) -> Element {
    rsx! {
        document::Title { "New Track" }
        h1 { "New Track" }
        SuspenseBoundary {
            fallback: |_context: SuspenseContext| rsx! { Loading {} },
            CreateTrackBody {
                artist_slug,
                release_slug,
                bad,
            }
        }
    }
}

#[component]
fn CreateTrackBody(artist_slug: String, release_slug: String, bad: Option<String>) -> Element {
    let session = use_user()?;
    let user = session().ok().flatten();
    if !permission_allows(user.as_ref(), "label_owner") {
        redirect(Route::Dashboard {});
        return rsx! { Loading {} };
    }

    // The page URL addresses the artist, so the pickers start on it.
    let artist_name_slug = artist_slug.clone();
    let artist_resource = use_page(move || page_data(get_artist(artist_name_slug.clone())))?;
    let artist = match artist_resource() {
        Ok(result) => result.artist,
        Err(error) => {
            tracing::error!(
                "Error while getting artist for the track create page (slug {artist_slug}): {error}"
            );
            redirect(Route::NotFound {
                segments: vec!["admin".into(), "artist-not-found".into()],
            });
            return rsx! { Loading {} };
        }
    };

    // The hidden `release_id` comes from the addressed release; a bad
    // release slug steers to the releases list.
    let release_artist_slug = artist_slug.clone();
    let release_slug_for_resource = release_slug.clone();
    let release_resource = use_page(move || {
        page_data(get_release(
            release_artist_slug.clone(),
            release_slug_for_resource.clone(),
        ))
    })?;
    let release = match release_resource() {
        Ok(result) => result.release,
        Err(error) => {
            tracing::error!(
                "Error while getting release for the track create page (artist {artist_slug}, release {release_slug}): {error}"
            );
            redirect(Route::Releases { artist_slug });
            return rsx! { Loading {} };
        }
    };
    let release_id = release.id;

    // Reactive create (slice 6i): the twin answers the new row and the page
    // navigates to its editor, which is what the 303 does without a client
    // build. The router is looked up rather than hooked (`use_navigator` would
    // panic) because the routerless SSR markup tests render this page alone.
    let router = try_use_context::<RouterContext>();
    let mut create =
        use_action(move |fields: TrackCreateFields| async move { create_track(fields).await });
    let new_track_artist = artist_slug.clone();
    let new_track_release = release_slug.clone();
    use_effect(move || {
        // The action resolves only after the submit handler has returned, so
        // leaving the page happens in an effect (as on the auth pages).
        let (Some(router), Some(result)) = (router, saved_row(create.value())) else {
            return;
        };
        // The only kind of failure is an external URL, which a `Route` is not.
        let _failure = router.replace(Route::EditTrack {
            artist_slug: new_track_artist.clone(),
            release_slug: new_track_release.clone(),
            track_slug: result.track.slug,
            updated: None,
            bad: None,
        });
    });
    let create_message = refused_code(create.value())
        .and_then(|code| track_message(Some(code.as_str())).map(str::to_string))
        .or_else(|| transport_message(create.value()));

    rsx! {
        form { method: "post", action: "/admin/forms/track",
            onsubmit: move |evt| {
                evt.prevent_default();
                create.call(posted_fields_with_ids::<TrackCreateFields>(
                    &evt,
                    "artist_ids",
                ));
            },
            div { class: "grid gap-6",
                if let Some(message) = track_message(bad.as_deref()) {
                    InlineError { message: message.to_string() }
                }
                if let Some(message) = create_message {
                    // Same fixed-code table the `?bad=` redirect path uses.
                    InlineError { message }
                }
                input { r#type: "hidden", name: "release_id", value: release_id.to_string() }
                input { r#type: "hidden", name: "artist_slug", value: artist_slug }
                input { r#type: "hidden", name: "release_slug", value: release_slug }
                label { class: "flex gap-2 items-center input",
                    input {
                        r#type: "text",
                        class: "grow",
                        placeholder: "Track name",
                        name: "name",
                    }
                }
                MarkdownField {
                    title: "Description".to_string(),
                    field: "description".to_string(),
                    markdown_text: String::new(),
                }
                MarkdownField {
                    title: "Lyrics".to_string(),
                    field: "lyrics".to_string(),
                    markdown_text: String::new(),
                }
                SuspenseBoundary {
                    fallback: |_context: SuspenseContext| rsx! { Loading {} },
                    ArtistSelect {
                        primary_artist_id: artist.id,
                        selected_ids: vec![artist.id],
                    }
                }
                label { class: "flex gap-2 items-center input",
                    input {
                        r#type: "text",
                        class: "grow",
                        placeholder: "ISRC Code",
                        name: "isrc_code",
                    }
                }
                label { class: "flex gap-2 items-center input",
                    input {
                        r#type: "number",
                        min: "0",
                        max: "999",
                        class: "grow",
                        placeholder: "BPM",
                        name: "bpm",
                    }
                }
                label { class: "flex gap-2 items-center input",
                    input {
                        r#type: "number",
                        min: "0",
                        max: "999",
                        class: "grow",
                        placeholder: "Track Number",
                        name: "track_number",
                        value: "0",
                    }
                }
                div { class: "flex gap-6",
                    DateField {
                        title: "Published At".to_string(),
                        field: "published_at".to_string(),
                        date: None,
                    }
                }
                button { class: "btn btn-primary", "Create" }
            }
        }
    }
}

/// Edit a track; `updated` and `bad` carry the handler codes from
/// update/delete/restore redirects.
#[component]
pub fn EditTrack(
    /// The artist slug from the URL.
    artist_slug: String,
    /// The release slug from the URL.
    release_slug: String,
    /// The track slug from the URL.
    track_slug: String,
    /// Set when the track has just been saved/deleted/restored.
    updated: Option<String>,
    /// Error code from a failed save/delete/restore.
    bad: Option<String>,
) -> Element {
    rsx! {
        SuspenseBoundary {
            fallback: |_context: SuspenseContext| rsx! { Loading {} },
            EditTrackBody {
                artist_slug,
                release_slug,
                track_slug,
                updated,
                bad,
            }
        }
    }
}

#[component]
fn EditTrackBody(
    artist_slug: String,
    release_slug: String,
    track_slug: String,
    updated: Option<String>,
    bad: Option<String>,
) -> Element {
    let session = use_user()?;
    let user = session().ok().flatten();
    if !permission_allows(user.as_ref(), "label_owner") {
        redirect(Route::Dashboard {});
        return rsx! { Loading {} };
    }

    // The header is "Track - Release - Artist", so all three rows are
    // needed; a bad slug steers where the missing entity is managed (a
    // missing release goes to the releases list, a missing track to the
    // tracks list; a missing artist gets the NotFound page, as in the other
    // editors).
    let header_artist_slug = artist_slug.clone();
    let artist_resource = use_page(move || page_data(get_artist(header_artist_slug.clone())))?;
    let artist = match artist_resource() {
        Ok(result) => result.artist,
        Err(error) => {
            tracing::error!(
                "Error while getting artist for the track editor (slug {artist_slug}): {error}"
            );
            redirect(Route::NotFound {
                segments: vec!["admin".into(), "artist-not-found".into()],
            });
            return rsx! { Loading {} };
        }
    };

    let track_release_artist_slug = artist_slug.clone();
    let track_release_slug = release_slug.clone();
    let release_resource = use_page(move || {
        page_data(get_release(
            track_release_artist_slug.clone(),
            track_release_slug.clone(),
        ))
    })?;
    let release = match release_resource() {
        Ok(result) => result.release,
        Err(error) => {
            tracing::error!(
                "Error while getting release for the track editor (artist {artist_slug}, release {release_slug}): {error}"
            );
            redirect(Route::Releases { artist_slug });
            return rsx! { Loading {} };
        }
    };
    let release_id = release.id;
    let primary_artist_id = release.primary_artist_id;

    let track_artist_slug = artist_slug.clone();
    let track_release_slug = release_slug.clone();
    let track_slug_for_resource = track_slug.clone();
    let track_resource = use_page(move || {
        page_data(get_track(
            track_artist_slug.clone(),
            track_release_slug.clone(),
            track_slug_for_resource.clone(),
        ))
    })?;
    let fetched = match track_resource() {
        Ok(result) => result,
        Err(error) => {
            tracing::error!(
                "Error while getting track for the editor (artist {artist_slug}, release {release_slug}, track {track_slug}): {error}"
            );
            redirect(Route::Tracks {
                artist_slug,
                release_slug,
            });
            return rsx! { Loading {} };
        }
    };

    // Reactive saves (slice 6i): update, delete and restore all go through one
    // action, so "the last thing the user did" is a single answer to read. With
    // a client build the `onsubmit` handlers below prevent the post and call
    // the twins at the bottom of this module; the plain form posts stay as the
    // javascript-off path (and are the only path a browser without one has).
    let mut edit =
        use_action(move |request: TrackEdit| async move { apply_track_edit(request).await });
    let saved = saved_row(edit.value());
    let just_saved = saved.is_some();
    // The row on screen follows the last save: a rename brings its new slug
    // (so the next save, the Delete button and the Restore button all target
    // that row), and a soft-delete swaps Delete for Restore without a page
    // load.
    let result = saved.unwrap_or(fetched);
    let reactive_message = refused_code(edit.value())
        .and_then(|code| track_message(Some(code.as_str())).map(str::to_string))
        .or_else(|| transport_message(edit.value()));

    let track = result.track;
    let selected_ids = result.artists.iter().map(|a| a.id).collect::<Vec<i64>>();
    let current_slug = track.slug.clone();
    let isrc_value = track.isrc_code.clone().unwrap_or_default();
    let bpm_value = track.bpm.map(|bpm| bpm.to_string()).unwrap_or_default();
    let track_number_value = track.track_number.to_string();
    let name_for_header = track.name.clone();
    let release_name = release.name;
    let artist_name = artist.name;

    rsx! {
        document::Title { "{name_for_header} - {release_name} - {artist_name}" }
        h1 { "{name_for_header} - {release_name} - {artist_name}" }
        Success {
            message: format!("{} Updated!", track.name),
            show: updated.is_some() || just_saved,
        }
        if let Some(message) = track_message(bad.as_deref()) {
            InlineError { message: message.to_string() }
        }
        if let Some(message) = reactive_message {
            // Same fixed-code table the `?bad=` redirect path uses.
            InlineError { message }
        }

        form { method: "post", action: "/admin/forms/track/update",
            onsubmit: move |evt| {
                evt.prevent_default();
                let fields = posted_fields_with_ids::<TrackUpdateFields>(&evt, "artist_ids");
                edit.call(TrackEdit::Update(fields));
            },
            div { class: "grid gap-6",
                input { r#type: "hidden", name: "release_id", value: release_id.to_string() }
                input { r#type: "hidden", name: "slug", value: current_slug }
                input { r#type: "hidden", name: "artist_slug", value: artist_slug }
                input { r#type: "hidden", name: "release_slug", value: release_slug }
                label { class: "flex gap-2 items-center input",
                    input {
                        r#type: "text",
                        class: "grow",
                        placeholder: "Track name",
                        name: "name",
                        value: track.name,
                    }
                }
                MarkdownField {
                    title: "Description".to_string(),
                    field: "description".to_string(),
                    markdown_text: track.description.clone(),
                }
                MarkdownField {
                    title: "Lyrics".to_string(),
                    field: "lyrics".to_string(),
                    markdown_text: track.lyrics.clone(),
                }
                SuspenseBoundary {
                    fallback: |_context: SuspenseContext| rsx! { Loading {} },
                    ArtistSelect {
                        primary_artist_id,
                        selected_ids,
                    }
                }
                label { class: "flex gap-2 items-center input",
                    input {
                        r#type: "text",
                        class: "grow",
                        placeholder: "ISRC Code",
                        name: "isrc_code",
                        value: isrc_value,
                    }
                }
                label { class: "flex gap-2 items-center input",
                    input {
                        r#type: "number",
                        min: "0",
                        max: "999",
                        class: "grow",
                        placeholder: "BPM",
                        name: "bpm",
                        value: bpm_value,
                    }
                }
                label { class: "flex gap-2 items-center input",
                    input {
                        r#type: "number",
                        min: "0",
                        max: "999",
                        class: "grow",
                        placeholder: "Track Number",
                        name: "track_number",
                        value: track_number_value,
                    }
                }
                div { class: "flex gap-6",
                    DateField {
                        title: "Published At".to_string(),
                        field: "published_at".to_string(),
                        date: track.published_at,
                    }
                }
                div { class: "flex flex-auto gap-6",
                    button { class: "flex-1 btn btn-primary", "Update" }
                    if track.deleted_at.is_some() {
                        button {
                            class: "btn btn-secondary",
                            form: "track-restore",
                            "Restore"
                        }
                    } else {
                        button { class: "btn btn-error", form: "track-delete", "Delete" }
                    }
                }
            }
        }
        // Sibling forms for the buttons above (a `<form>` may not nest);
        // they carry the artist and release slugs the redirects need.
        DeleteForm {
            slug: current_slug.clone(),
            artist_slug: artist_slug.clone(),
            release_slug: release_slug.clone(),
            onsubmit: move |evt: FormEvent| {
                evt.prevent_default();
                edit.call(TrackEdit::Delete(posted_fields::<TrackSlugFields>(
                    &evt,
                )));
            },
        }
        RestoreForm {
            slug: current_slug.clone(),
            artist_slug: artist_slug.clone(),
            release_slug: release_slug.clone(),
            onsubmit: move |evt: FormEvent| {
                evt.prevent_default();
                edit.call(TrackEdit::Restore(posted_fields::<TrackSlugFields>(
                    &evt,
                )));
            },
        }
    }
}

/// The soft-delete sibling form (the `DeleteTrack` dialog's JS-free twin).
#[component]
fn DeleteForm(
    slug: String,
    artist_slug: String,
    release_slug: String,
    /// Reactive path: a client build soft-deletes through the twin, not a post.
    onsubmit: EventHandler<FormEvent>,
) -> Element {
    rsx! {
        form { id: "track-delete", method: "post", action: "/admin/forms/track/delete",
            onsubmit: onsubmit,
            input { r#type: "hidden", name: "slug", value: slug }
            input { r#type: "hidden", name: "artist_slug", value: artist_slug }
            input { r#type: "hidden", name: "release_slug", value: release_slug }
            // Keyboard-submit fallback; the visible trigger is the button
            // with `form="track-delete"`.
            button { class: "hidden", "Delete" }
        }
    }
}

/// The restore sibling form (the `RestoreTrack` dialog's JS-free twin).
#[component]
fn RestoreForm(
    slug: String,
    artist_slug: String,
    release_slug: String,
    /// Reactive path: a client build restores through the twin, not a post.
    onsubmit: EventHandler<FormEvent>,
) -> Element {
    rsx! {
        form { id: "track-restore", method: "post", action: "/admin/forms/track/restore",
            onsubmit: onsubmit,
            input { r#type: "hidden", name: "slug", value: slug }
            input { r#type: "hidden", name: "artist_slug", value: artist_slug }
            input { r#type: "hidden", name: "release_slug", value: release_slug }
            button { class: "hidden", "Restore" }
        }
    }
}

/// The on-screen message for the track form's fixed error codes; unknown
/// codes and absent codes render nothing. Mirrors the service and model
/// validation messages (`crate::server::forms` picks the codes).
fn track_message(code: Option<&str>) -> Option<&'static str> {
    match code {
        Some("forbidden") => Some("You do not have permission."),
        Some("name_required") => Some("Name is required."),
        Some("name_length") => Some("Name must be less than 255 characters."),
        Some("slug_taken") => Some("Slug must be unique."),
        Some("no_artist") => Some("Artist does not exist."),
        Some("no_release") => Some("Release does not exist."),
        Some("isrc_length") => Some("ISRC code must be 12 characters."),
        Some("isrc_taken") => Some("ISRC code must be unique."),
        Some("track_number_taken") => Some("Track number must be unique for the release."),
        Some("server") => Some("Something went wrong, please try again later."),
        Some(_) | None => None,
    }
}

// ---------------------------------------------------------------------------
// Reactive twins of the `/admin/forms/track*` handlers (slice 6i)
// ---------------------------------------------------------------------------
//
// Same services and same fixed codes as the javascript-off handlers in
// `crate::server::forms`; only the delivery differs. A client build has no page
// load to carry `?updated=1` / `?bad=<code>`, and a webview window cannot
// follow a 303 at all, so a refusal answers with the very same code as data and
// a success answers the page's own `TrackResult`:
//
// ```text
// POST /api/update_track  {"fields":{"slug":"a-track","name":"B",…}}
// 200 {"Saved":{"track":{"slug":"b",…},"artists":[…]}}
//
// POST /api/update_track  {"fields":{"slug":"a-track","name":"",…}}
// 200 {"Failed":{"code":"name_required"}}
// ```
//
// They live on `/api/*_track` rather than on the form handlers' `/admin/forms/…` paths: axum
// allows only one handler per method and path.

/// What the track editor asks its action to do.
///
/// Named `TrackEdit`, not `EditTrack` like the component: the component macro
/// already puts an `EditTrack` in the module's type namespace (its completion
/// hint), so the two names would collide — `super::release` resolves it the
/// same way.
///
/// All three saves share one action so the page has a single answer to read —
/// which of three action signals fired most recently is otherwise guesswork.
#[expect(
    clippy::large_enum_variant,
    reason = "the update form is the variant the editor reaches for most, and the action's argument moves straight into the twin, so boxing it to shrink the two slug variants by a few hundred bytes would only add a dereference"
)]
enum TrackEdit {
    Update(TrackUpdateFields),
    Delete(TrackSlugFields),
    Restore(TrackSlugFields),
}

/// Runs one track-editor action through its twin. On a client build these
/// calls reach the generated stubs, so this lives on both sides.
async fn apply_track_edit(
    request: TrackEdit,
) -> core::result::Result<EditOutcome<TrackResult>, HttpError> {
    match request {
        TrackEdit::Update(fields) => update_track(fields).await,
        TrackEdit::Delete(target) => delete_track(target).await,
        TrackEdit::Restore(target) => restore_track(target).await,
    }
}

/// Creates a track, answering the new row so the create page can navigate to
/// its editor (what the 303 handler does with its `Location`).
///
/// # Errors
/// 401 when the session is gone, 500 with a retry message when the create could
/// not run at all. Refusals (validation, permissions) answer `Failed`.
#[post("/api/create_track",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser
)]
pub async fn create_track(
    fields: TrackCreateFields,
) -> core::result::Result<EditOutcome<TrackResult>, HttpError> {
    let Some(session_user) = user.0.as_ref() else {
        // The page gate redirects guests before they reach this form; a
        // hand-crafted POST gets the steer the 303 handler would have given.
        return HttpError::unauthorized("Please log in to save.");
    };
    match services::track::create_track_service(
        &db.0,
        Some(session_user),
        fields.into_service_form(),
    )
    .await
    {
        Ok(result) => Ok(EditOutcome::Saved(result)),
        Err(error) => {
            tracing::warn!("track create twin rejected: {error}");
            Ok(EditOutcome::Failed {
                code: track_code(&error).to_string(),
            })
        }
    }
}

/// Saves a track.
///
/// A rename re-sluggifies, and the answer carries the row's *current* slug,
/// which is what the editor then keeps saving to.
///
/// # Errors
/// 401 when the session is gone, 500 with a retry message when the save could
/// not run at all.
#[post("/api/update_track",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser
)]
pub async fn update_track(
    fields: TrackUpdateFields,
) -> core::result::Result<EditOutcome<TrackResult>, HttpError> {
    let Some(session_user) = user.0.as_ref() else {
        return HttpError::unauthorized("Please log in to save.");
    };
    match services::track::update_track_service(
        &db.0,
        Some(session_user),
        fields.into_service_form(),
    )
    .await
    {
        Ok(result) => Ok(EditOutcome::Saved(result)),
        Err(error) => {
            tracing::warn!("track update twin rejected: {error}");
            Ok(EditOutcome::Failed {
                code: track_code(&error).to_string(),
            })
        }
    }
}

/// Soft-deletes a track; the row in the answer carries `deleted_at`, which is
/// what swaps the editor's Delete button for Restore.
///
/// # Errors
/// 401 when the session is gone, 500 with a retry message when the delete could
/// not run at all.
#[post("/api/delete_track",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser
)]
pub async fn delete_track(
    target: TrackSlugFields,
) -> core::result::Result<EditOutcome<TrackResult>, HttpError> {
    let Some(session_user) = user.0.as_ref() else {
        return HttpError::unauthorized("Please log in to save.");
    };
    match services::track::delete_track_service(&db.0, Some(session_user), target.slug).await {
        Ok(result) => Ok(EditOutcome::Saved(result)),
        Err(error) => {
            tracing::warn!("track delete twin rejected: {error}");
            Ok(EditOutcome::Failed {
                code: track_code(&error).to_string(),
            })
        }
    }
}

/// Restores a soft-deleted track.
///
/// # Errors
/// 401 when the session is gone, 500 with a retry message when the restore
/// could not run at all.
#[post("/api/restore_track",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser
)]
pub async fn restore_track(
    target: TrackSlugFields,
) -> core::result::Result<EditOutcome<TrackResult>, HttpError> {
    let Some(session_user) = user.0.as_ref() else {
        return HttpError::unauthorized("Please log in to save.");
    };
    match services::track::restore_track_service(&db.0, Some(session_user), target.slug).await {
        Ok(result) => Ok(EditOutcome::Saved(result)),
        Err(error) => {
            tracing::warn!("track restore twin rejected: {error}");
            Ok(EditOutcome::Failed {
                code: track_code(&error).to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::track_message;

    #[test]
    fn track_messages_cover_the_known_codes() {
        assert_eq!(
            track_message(Some("forbidden")),
            Some("You do not have permission.")
        );
        assert_eq!(
            track_message(Some("name_required")),
            Some("Name is required.")
        );
        assert_eq!(
            track_message(Some("name_length")),
            Some("Name must be less than 255 characters.")
        );
        assert_eq!(
            track_message(Some("slug_taken")),
            Some("Slug must be unique.")
        );
        assert_eq!(
            track_message(Some("no_artist")),
            Some("Artist does not exist.")
        );
        assert_eq!(
            track_message(Some("no_release")),
            Some("Release does not exist.")
        );
        assert_eq!(
            track_message(Some("isrc_length")),
            Some("ISRC code must be 12 characters.")
        );
        assert_eq!(
            track_message(Some("isrc_taken")),
            Some("ISRC code must be unique.")
        );
        assert_eq!(
            track_message(Some("track_number_taken")),
            Some("Track number must be unique for the release.")
        );
        assert_eq!(
            track_message(Some("server")),
            Some("Something went wrong, please try again later.")
        );
    }

    #[test]
    fn unknown_and_missing_track_codes_render_nothing() {
        assert_eq!(track_message(Some("<script>alert(1)</script>")), None);
        assert_eq!(track_message(None), None);
    }
}
