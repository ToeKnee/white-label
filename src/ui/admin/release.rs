//! The releases list and editors (MIGRATION.md Phase 4 slice 5b), ported
//! from the Leptos `components::admin::artist::release` group plus the
//! shared `artist_select` component.
//!
//! Like the artist editors (`super::artist`) these pages post to plain-HTML
//! handlers in `crate::server::forms`: `POST /admin/forms/release` creates,
//! `.../release/update` updates, and `.../release/delete` / `.../restore`
//! soft-delete and restore. The handlers 303 with fixed query codes
//! (`?bad=<code>` on failure, `?updated=1` on success); `release_message`
//! maps the codes to text mirroring the service's messages.
//!
//! Deviations from the Leptos originals:
//!
//! - `ArtistSelect` is JS-free: the per-artist checkboxes *are* the form
//!   fields (repeated `artist_ids` entries), replacing the original's
//!   hidden comma-joined input that click handlers updated. The primary
//!   artist stays a `<select>`.
//! - The editor's image uploader (`FileUploadWithProgress`, wired up in
//!   Phase 5 slice 5g) sits *after* the field form instead of between
//!   "Release Date" and "Private" as Leptos had it: the uploader brings its
//!   own `<form>`, and a form may not nest — the Leptos page relied on the
//!   browser dropping the inner `<form>` tag, which also closed the outer one
//!   early. The `<img>` follows a finished upload at once (Leptos kept showing
//!   the old file until a reload).
//! - The list's "Tracks" column keeps the original's static `0` (the list
//!   service carries no track counts), and the editor's sidebar renders the
//!   real track list from the same `get_release` fetch instead of Leptos'
//!   second `get_tracks` request.
//!
//! The page gate is the `label_owner` permission; the release services also
//! require `admin` (a `label_owner` without it gets the `forbidden` alert
//! on submit — same behaviour as the original).

use dioxus::prelude::*;

use crate::config::upload::UploadConfiguration;
use crate::models::artist::Artist;
use crate::models::release::Release;
use crate::models::track::Track;
use crate::ui::assets::display_image_url;
use crate::ui::data::{get_artist, get_label_artists, get_release, get_releases};
use crate::ui::page_data;
use crate::ui::upload::{FileUploadWithProgress, stored_file_url};
use crate::ui::use_label;
use crate::ui::use_page;
use crate::ui::use_user;
use crate::ui::utils::{ErrorPage, InlineError, Loading, StatusBadge, Success, permission_allows};
use crate::ui::{Route, redirect};

use super::pages::{DateField, MarkdownField};
use super::track::TrackList;

/// The list of an artist's releases (Leptos `release/list.rs`).
#[component]
pub fn Releases(
    /// The artist slug from the URL.
    artist_slug: String,
) -> Element {
    rsx! {
        SuspenseBoundary {
            fallback: |_context: SuspenseContext| rsx! { Loading {} },
            ReleasesBody { artist_slug }
        }
    }
}

#[component]
fn ReleasesBody(artist_slug: String) -> Element {
    let session = use_user()?;
    let user = session().ok().flatten();
    if !permission_allows(user.as_ref(), "label_owner") {
        redirect(Route::Dashboard {});
        return rsx! { Loading {} };
    }

    // The header shows the artist's name; a slug that matches no artist
    // redirects to NotFound (the same unmatched target as the artist
    // editor — see `EditArtistBody` in `super::artist`).
    let artist_name_slug = artist_slug.clone();
    let artist_resource = use_page(move || page_data(get_artist(artist_name_slug.clone())))?;
    let artist = match artist_resource() {
        Ok(result) => result.artist,
        Err(error) => {
            tracing::error!(
                "Error while getting artist for the releases list (slug {artist_slug}): {error}"
            );
            redirect(Route::NotFound {
                segments: vec!["admin".into(), "artist-not-found".into()],
            });
            return rsx! { Loading {} };
        }
    };

    // `label_owner` sessions get hidden (unpublished/soft-deleted) rows too,
    // through the service's visibility check.
    let releases_slug = artist_slug.clone();
    let releases_resource = use_page(move || page_data(get_releases(releases_slug.clone())))?;
    let releases = match releases_resource() {
        Ok(result) => result.releases,
        Err(error) => {
            tracing::error!(
                "Error while getting releases for the list (slug {artist_slug}): {error}"
            );
            return rsx! { ErrorPage {} };
        }
    };

    let add_route = Route::CreateRelease {
        artist_slug: artist_slug.clone(),
        bad: None,
    };

    rsx! {
        document::Title { "{artist.name} Releases" }
        h1 { "{artist.name} Releases" }
        div { class: "overflow-x-auto",
            table { class: "table",
                thead {
                    tr {
                        th {}
                        th { "Name" }
                        th { "Tracks" }
                        th { "Release Date" }
                        th {}
                    }
                }
                tbody {
                    if releases.is_empty() {
                        tr { td { colspan: 5, "No releases found." } }
                    } else {
                        for release in releases {
                            ReleaseRow {
                                release,
                                artist_slug: artist_slug.clone(),
                            }
                        }
                    }
                    tr {
                        td { colspan: 4, "" }
                        td {
                            Link { to: add_route, class: "btn btn-primary", "Add" }
                        }
                    }
                }
                tfoot {
                    tr {
                        th {}
                        th { "Name" }
                        th { "Tracks" }
                        th { "Release Date" }
                        th {}
                    }
                }
            }
        }
    }
}

/// One table row (the Leptos `ReleaseRow`). The "Tracks" count stays the
/// original's static `0` until the track editors land (slice 5c).
#[component]
fn ReleaseRow(release: Release, artist_slug: String) -> Element {
    let release_date = release.release_date.map_or_else(
        || "Unreleased".to_string(),
        |date| date.format("%e %B %Y").to_string(),
    );
    let edit_route = Route::EditRelease {
        artist_slug,
        release_slug: release.slug.clone(),
        updated: None,
        bad: None,
        uploaded: None,
        bad_upload: None,
    };

    rsx! {
        tr {
            th {
                StatusBadge {
                    deleted_at: release.deleted_at,
                    published_at: release.published_at,
                }
            }
            td {
                div { class: "flex gap-3 items-center",
                    div { class: "avatar",
                        div { class: "w-12 rounded-full not-prose",
                            img {
                                class: "m-0",
                                src: display_image_url(&release.primary_image_url()),
                                alt: release.name,
                            }
                        }
                    }
                    div {
                        div { class: "font-bold",
                            Link { to: edit_route.clone(), "{release.name}" }
                        }
                        div { class: "text-sm opacity-50",
                            "{release.catalogue_number}"
                            br {}
                            " {release.slug}"
                        }
                    }
                }
            }
            td { "0" }
            td { "{release_date}" }
            td {
                Link { to: edit_route, class: "btn btn-primary", "Edit" }
            }
        }
    }
}

/// Create a release for an artist; `bad` carries the create handler's code.
#[component]
pub fn CreateRelease(
    /// The artist slug from the URL.
    artist_slug: String,
    /// Error code from a failed create.
    bad: Option<String>,
) -> Element {
    rsx! {
        document::Title { "New Release" }
        h1 { "New Release" }
        SuspenseBoundary {
            fallback: |_context: SuspenseContext| rsx! { Loading {} },
            CreateReleaseBody { artist_slug, bad }
        }
    }
}

#[component]
fn CreateReleaseBody(artist_slug: String, bad: Option<String>) -> Element {
    let session = use_user()?;
    let user = session().ok().flatten();
    if !permission_allows(user.as_ref(), "label_owner") {
        redirect(Route::Dashboard {});
        return rsx! { Loading {} };
    }

    // The page URL addresses the artist, so the form starts with it as the
    // primary/selected artist (the Leptos effect that seeded `artist_ids`).
    let artist_name_slug = artist_slug.clone();
    let artist_resource = use_page(move || page_data(get_artist(artist_name_slug.clone())))?;
    let artist = match artist_resource() {
        Ok(result) => result.artist,
        Err(error) => {
            tracing::error!(
                "Error while getting artist for the release create page (slug {artist_slug}): {error}"
            );
            redirect(Route::NotFound {
                segments: vec!["admin".into(), "artist-not-found".into()],
            });
            return rsx! { Loading {} };
        }
    };

    // The release is stamped with the label's id, like the Leptos hidden
    // `label_id` field.
    let label_id = match use_label()?() {
        Ok(result) => result.record_label.id,
        Err(error) => {
            tracing::error!("Error while getting label for release create: {error}");
            0
        }
    };

    rsx! {
        form { method: "post", action: "/admin/forms/release",
            div { class: "grid gap-6",
                if let Some(message) = release_message(bad.as_deref()) {
                    InlineError { message: message.to_string() }
                }
                input { r#type: "hidden", name: "label_id", value: label_id.to_string() }
                input { r#type: "hidden", name: "artist_slug", value: artist_slug }
                label { class: "flex gap-2 items-center input",
                    input {
                        r#type: "text",
                        class: "grow",
                        placeholder: "Release name",
                        name: "name",
                    }
                }
                MarkdownField {
                    title: "Description".to_string(),
                    field: "description".to_string(),
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
                        placeholder: "Catalog number",
                        name: "catalogue_number",
                    }
                }
                div { class: "flex gap-6",
                    div { class: "w-1/2",
                        DateField {
                            title: "Release Date".to_string(),
                            field: "release_date".to_string(),
                            date: None,
                        }
                    }
                    div { class: "w-1/2",
                        DateField {
                            title: "Published At".to_string(),
                            field: "published_at".to_string(),
                            date: None,
                        }
                    }
                }
                button { class: "btn btn-primary", "Create" }
            }
        }
    }
}

/// Edit a release; `updated` and `bad` carry the handler codes from
/// update/delete/restore redirects, `uploaded`/`bad_upload` those from the
/// image upload form.
#[component]
pub fn EditRelease(
    /// The artist slug from the URL.
    artist_slug: String,
    /// The release slug from the URL.
    release_slug: String,
    /// Set when the release has just been saved/deleted/restored.
    updated: Option<String>,
    /// Error code from a failed save/delete/restore.
    bad: Option<String>,
    /// Set when an image arrived through the javascript-off upload form.
    uploaded: Option<String>,
    /// Error code from a failed javascript-off upload.
    bad_upload: Option<String>,
) -> Element {
    rsx! {
        SuspenseBoundary {
            fallback: |_context: SuspenseContext| rsx! { Loading {} },
            EditReleaseBody {
                artist_slug,
                release_slug,
                updated,
                bad,
                uploaded,
                bad_upload,
            }
        }
    }
}

#[component]
fn EditReleaseBody(
    artist_slug: String,
    release_slug: String,
    updated: Option<String>,
    bad: Option<String>,
    uploaded: Option<String>,
    bad_upload: Option<String>,
) -> Element {
    let session = use_user()?;
    let user = session().ok().flatten();
    if !permission_allows(user.as_ref(), "label_owner") {
        redirect(Route::Dashboard {});
        return rsx! { Loading {} };
    }

    // The artist carries the header name and the hidden `label_id` (the
    // Leptos form read both from the store).
    let artist_name_slug = artist_slug.clone();
    let artist_resource = use_page(move || page_data(get_artist(artist_name_slug.clone())))?;
    let artist = match artist_resource() {
        Ok(result) => result.artist,
        Err(error) => {
            tracing::error!(
                "Error while getting artist for the release editor (slug {artist_slug}): {error}"
            );
            redirect(Route::NotFound {
                segments: vec!["admin".into(), "artist-not-found".into()],
            });
            return rsx! { Loading {} };
        }
    };

    // `label_owner` sessions see soft-deleted rows too (the service's
    // visibility check), so the Restore button can appear.
    let release_artist_slug = artist_slug.clone();
    let release_slug_for_resource = release_slug.clone();
    let release_resource = use_page(move || {
        page_data(get_release(
            release_artist_slug.clone(),
            release_slug_for_resource.clone(),
        ))
    })?;
    let result = match release_resource() {
        Ok(result) => result,
        Err(error) => {
            tracing::error!(
                "Error while getting release for the editor (artist {artist_slug}, release {release_slug}): {error}"
            );
            redirect(Route::NotFound {
                segments: vec!["admin".into(), "artist-not-found".into()],
            });
            return rsx! { Loading {} };
        }
    };
    let release = result.release;
    // Read the image URL and alt text before the form takes the row apart
    // field by field (the uploader's `<img>` needs both).
    let current_image_url = display_image_url(&release.primary_image_url());
    let image_alt = release.name.clone();
    // The sidebar track list shares this fetch (Leptos made a second
    // `get_tracks` request for it).
    let sidebar_tracks = result
        .tracks
        .iter()
        .map(|track_with_artists| track_with_artists.track.clone())
        .collect::<Vec<Track>>();
    let selected_ids = result.artists.iter().map(|a| a.id).collect::<Vec<i64>>();
    let primary_artist_id = release.primary_artist_id;
    let current_slug = release.slug.clone();

    // Set when an upload lands, so the `<img>` shows the new file without a
    // reload. Read here in the body, which is what re-renders on it.
    let mut uploaded_image = use_signal(|| None::<String>);
    let shown_image = uploaded_image().map_or(current_image_url, |stored| {
        stored_file_url(UploadConfiguration::Release, &stored)
    });

    rsx! {
        div { class: "flex gap-6 justify-around w-full",
            div { class: "w-2/3",
                document::Title { "{release.name} - {artist.name}" }
                h1 { "{release.name} - {artist.name}" }
        Success {
            message: format!("{} Updated!", release.name),
            show: updated.is_some(),
        }
        if let Some(message) = release_message(bad.as_deref()) {
            InlineError { message: message.to_string() }
        }

        form { method: "post", action: "/admin/forms/release/update",
            div { class: "grid gap-6",
                input { r#type: "hidden", name: "artist_slug", value: artist_slug }
                input { r#type: "hidden", name: "label_id", value: artist.label_id.to_string() }
                input { r#type: "hidden", name: "slug", value: current_slug }
                div { class: "divider", "Public" }
                label { class: "flex gap-2 items-center input",
                    input {
                        r#type: "text",
                        class: "grow",
                        placeholder: "Release name",
                        name: "name",
                        value: release.name.clone(),
                    }
                }
                MarkdownField {
                    title: "Description".to_string(),
                    field: "description".to_string(),
                    markdown_text: release.description.clone(),
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
                        placeholder: "Catalog number",
                        name: "catalogue_number",
                        value: release.catalogue_number,
                    }
                }
                DateField {
                    title: "Release Date".to_string(),
                    field: "release_date".to_string(),
                    date: release.release_date,
                }
                div { class: "divider", "Private" }
                DateField {
                    title: "Published At".to_string(),
                    field: "published_at".to_string(),
                    date: release.published_at,
                }
                div { class: "flex flex-auto gap-6",
                    button { class: "flex-1 btn btn-primary", "Update" }
                    if release.deleted_at.is_some() {
                        button {
                            class: "btn btn-secondary",
                            form: "release-restore",
                            "Restore"
                        }
                    } else {
                        button { class: "btn btn-error", form: "release-delete", "Delete" }
                    }
                }
            }
        }
        // (slice 5g) The image uploader. Leptos put it between "Release
        // Date" and "Private", but it brings its own `<form>` and forms may
        // not nest, so it lives after the field form here (module docs).
        div { class: "divider", "Images" }
        FileUploadWithProgress {
            config: UploadConfiguration::Release,
            slug: current_slug.clone(),
            // The javascript-off handler needs the artist too: its editor sits
            // under the artist route.
            artist_slug: Some(artist_slug.clone()),
            uploaded,
            bad_upload,
            on_uploaded: move |stored_name: String| uploaded_image.set(Some(stored_name)),
        }
        img { src: shown_image, alt: image_alt }
        // Sibling forms for the buttons above (a `<form>` may not nest);
        // they carry the artist slug the redirect target needs.
        DeleteForm {
            slug: current_slug.clone(),
            artist_slug: artist_slug.clone(),
        }
        RestoreForm {
            slug: current_slug.clone(),
            artist_slug: artist_slug.clone(),
        }
            }
            div { class: "w-1/3",
                // Slice 5c: the sidebar is the real track list now, fed by
                // the same fetch as the form.
                TrackList {
                    artist_slug: artist_slug.clone(),
                    release_slug,
                    tracks: sidebar_tracks,
                }
            }
        }
    }
}

/// The soft-delete sibling form (the `DeleteRelease` dialog's JS-free twin).
#[component]
fn DeleteForm(slug: String, artist_slug: String) -> Element {
    rsx! {
        form { id: "release-delete", method: "post", action: "/admin/forms/release/delete",
            input { r#type: "hidden", name: "slug", value: slug }
            input { r#type: "hidden", name: "artist_slug", value: artist_slug }
            // Keyboard-submit fallback; the visible trigger is the button
            // with `form="release-delete"`.
            button { class: "hidden", "Delete" }
        }
    }
}

/// The restore sibling form (the `RestoreRelease` dialog's JS-free twin).
#[component]
fn RestoreForm(slug: String, artist_slug: String) -> Element {
    rsx! {
        form { id: "release-restore", method: "post", action: "/admin/forms/release/restore",
            input { r#type: "hidden", name: "slug", value: slug }
            input { r#type: "hidden", name: "artist_slug", value: artist_slug }
            button { class: "hidden", "Restore" }
        }
    }
}

/// The artist pickers (Leptos `admin::shared::artist_select`). Without JS
/// the checkboxes themselves carry the selection: every checked one posts an
/// `artist_ids` entry, and `crate::server::forms` joins them into the
/// service's comma-separated string.
#[component]
pub fn ArtistSelect(
    /// The release's primary artist; its option is selected in the
    /// "Main Artist" `<select>`.
    primary_artist_id: i64,
    /// The ids checked in the "All Artists" list.
    selected_ids: Vec<i64>,
) -> Element {
    rsx! {
        SuspenseBoundary {
            fallback: |_context: SuspenseContext| rsx! { Loading {} },
            ArtistSelectBody {
                primary_artist_id,
                selected_ids,
            }
        }
    }
}

#[component]
fn ArtistSelectBody(primary_artist_id: i64, selected_ids: Vec<i64>) -> Element {
    // The label's artists, visibility-filtered by the service (a
    // `label_owner` picks from every artist on the label).
    let artists_resource = use_page(move || page_data(get_label_artists()))?;
    let artists = match artists_resource() {
        Ok(result) => result.artists,
        Err(error) => {
            tracing::error!("Error while getting label artists for the release picker: {error}");
            return rsx! { ErrorPage {} };
        }
    };

    rsx! {
        div { class: "flex flex-col gap-4",
            h2 { "Artists" }
            legend { class: "label",
                span { class: "label-text", "Main Artist" }
            }
            select { class: "select", name: "primary_artist_id",
                for label_artist in &artists {
                    option {
                        class: "option",
                        value: label_artist.id.to_string(),
                        selected: primary_artist_id == label_artist.id,
                        "{label_artist.name}"
                    }
                }
            }

            fieldset { class: "flex flex-row flex-wrap gap-6 justify-center p-4 fieldset",
                legend { class: "label",
                    span { class: "label-text", "All Artists" }
                }
                if artists.is_empty() {
                    p { "No artists found…" }
                } else {
                    for label_artist in &artists {
                        ArtistCheckbox {
                            artist: label_artist.clone(),
                            selected: selected_ids.contains(&label_artist.id),
                        }
                    }
                }
            }
        }
    }
}

/// One artist's checkbox row (the Leptos `ArtistCheckbox`; its click
/// handler is unnecessary now — the checkbox *is* the form field).
#[component]
fn ArtistCheckbox(artist: Artist, selected: bool) -> Element {
    rsx! {
        label { class: "flex flex-row gap-4 label bg-base-100 border-base-300 rounded-box",
            input {
                class: "checkbox",
                r#type: "checkbox",
                name: "artist_ids",
                value: artist.id.to_string(),
                checked: selected,
            }
            div { class: "avatar not-prose",
                div { class: "w-8 rounded-full",
                    img { src: display_image_url(&artist.primary_image_url()), alt: artist.name }
                }
            }
            "{artist.name}"
        }
    }
}

/// The on-screen message for the release form's fixed error codes; unknown
/// codes and absent codes render nothing. Mirrors the service and model
/// validation messages (`crate::server::forms` picks the codes).
fn release_message(code: Option<&str>) -> Option<&'static str> {
    match code {
        Some("forbidden") => Some("You do not have permission."),
        Some("name_required") => Some("Name is required."),
        Some("name_length") => Some("Name must be less than 255 characters."),
        Some("slug_taken") => Some("Slug must be unique."),
        Some("no_label") => Some("Record Label does not exist."),
        Some("no_artist") => Some("Artist does not exist."),
        Some("catalogue_length") => Some("Catalogue number must be less than 255 characters."),
        Some("catalogue_taken") => Some("Catalogue number must be unique."),
        Some("server") => Some("Something went wrong, please try again later."),
        Some(_) | None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::release_message;

    #[test]
    fn release_messages_cover_the_known_codes() {
        assert_eq!(
            release_message(Some("forbidden")),
            Some("You do not have permission.")
        );
        assert_eq!(
            release_message(Some("name_required")),
            Some("Name is required.")
        );
        assert_eq!(
            release_message(Some("name_length")),
            Some("Name must be less than 255 characters.")
        );
        assert_eq!(
            release_message(Some("slug_taken")),
            Some("Slug must be unique.")
        );
        assert_eq!(
            release_message(Some("no_label")),
            Some("Record Label does not exist.")
        );
        assert_eq!(
            release_message(Some("no_artist")),
            Some("Artist does not exist.")
        );
        assert_eq!(
            release_message(Some("catalogue_length")),
            Some("Catalogue number must be less than 255 characters.")
        );
        assert_eq!(
            release_message(Some("catalogue_taken")),
            Some("Catalogue number must be unique.")
        );
        assert_eq!(
            release_message(Some("server")),
            Some("Something went wrong, please try again later.")
        );
    }

    #[test]
    fn unknown_and_missing_release_codes_render_nothing() {
        assert_eq!(release_message(Some("<script>alert(1)</script>")), None);
        assert_eq!(release_message(None), None);
    }
}
