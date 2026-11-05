//! The track list and editors (MIGRATION.md Phase 4 slice 5c), ported from
//! the Leptos `components::admin::artist::release::track` group.
//!
//! Same shape as the release editors (`super::release`): pages post to
//! `crate::server::forms` handlers under `/admin/forms/track`, the handlers
//! 303 with fixed query codes (`?updated=1`, `?bad=<code>`), and
//! `track_message` maps the codes to text mirroring the service/model
//! validation messages. The page gate is `label_owner`; the track services
//! additionally require `admin`.
//!
//! Deviations from the Leptos originals:
//!
//! - The track list (a `TrackList`) is rendered from the release editor's
//!   existing `get_release` result (whose `tracks` field already carries
//!   them) instead of a second `get_tracks` fetch; the list page reuses the
//!   same twin. A new `get_track` twin serves the track editor.
//! - Leptos' create-page `<Title>` said "New Release" (a copy-paste slip);
//!   the port titles it "New Track".
//! - The image upload block stays deferred to Phase 5, as elsewhere.

use dioxus::prelude::*;

use crate::models::track::Track;
use crate::ui::assets::display_image_url;
use crate::ui::data::{get_artist, get_release, get_track};
use crate::ui::page_data;
use crate::ui::use_page;
use crate::ui::use_user;
use crate::ui::utils::{InlineError, Loading, Success, permission_allows};
use crate::ui::{Route, redirect};

use super::pages::{DateField, MarkdownField};
use super::release::ArtistSelect;

/// The track list page for one release (Leptos `track/list.rs` at the
/// `/tracks` route).
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
/// the release editor's sidebar (where Leptos rendered the same component).
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

/// One track row (the Leptos `TrackRow`).
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
        // Leptos titled this page "New Release" (copy-paste slip); fixed.
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

    // The page URL addresses the artist, so the pickers start on it (the
    // Leptos effect that seeded `artist_ids`).
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
    // release slug steers to the releases list, as in Leptos.
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

    rsx! {
        form { method: "post", action: "/admin/forms/track",
            div { class: "grid gap-6",
                if let Some(message) = track_message(bad.as_deref()) {
                    InlineError { message: message.to_string() }
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
    // needed; the redirects mirror the Leptos error branches (a missing
    // release steers to the releases list, a missing track to the tracks
    // list; a missing artist gets the NotFound page, as in the other
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
    let result = match track_resource() {
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
            show: updated.is_some(),
        }
        if let Some(message) = track_message(bad.as_deref()) {
            InlineError { message: message.to_string() }
        }

        form { method: "post", action: "/admin/forms/track/update",
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
        }
        RestoreForm {
            slug: current_slug.clone(),
            artist_slug: artist_slug.clone(),
            release_slug: release_slug.clone(),
        }
    }
}

/// The soft-delete sibling form (the `DeleteTrack` dialog's JS-free twin).
#[component]
fn DeleteForm(slug: String, artist_slug: String, release_slug: String) -> Element {
    rsx! {
        form { id: "track-delete", method: "post", action: "/admin/forms/track/delete",
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
fn RestoreForm(slug: String, artist_slug: String, release_slug: String) -> Element {
    rsx! {
        form { id: "track-restore", method: "post", action: "/admin/forms/track/restore",
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
