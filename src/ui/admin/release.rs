//! The releases list and editors.
//!
//! Like the artist editors (`super::artist`) these pages post to plain-HTML
//! handlers in `crate::server::forms`: `POST /admin/forms/release` creates,
//! `.../release/update` updates, and `.../release/delete` / `.../restore`
//! soft-delete and restore. The handlers 303 with fixed query codes
//! (`?bad=<code>` on failure, `?updated=1` on success); `release_message`
//! maps the codes to text mirroring the service's messages.
//!
//! The editors also save **reactively**: with a client build `onsubmit`
//! prevents the post and one `use_action` calls the JSON twins at the bottom
//! of this module (`/api/*_release`), which answer the page's own
//! `ReleaseResult` (or the same fixed codes as data). The row on screen then
//! follows the save — a rename carries the new slug into the next save and
//! the uploader, and a soft-delete swaps Delete for Restore — with no page
//! load, which is what makes these editors usable in a desktop window (a
//! 303 has nothing to redirect there). The plain form posts stay underneath
//! as the javascript-off path.
//!
//! Design notes:
//!
//! - `ArtistSelect` is JS-free: the per-artist checkboxes *are* the form
//!   fields (repeated `artist_ids` entries). The primary artist stays a
//!   `<select>`.
//! - The editor's image uploader (`FileUploadWithProgress`) sits *after*
//!   the field form, not inside it: the uploader brings its own `<form>`,
//!   and a form may not nest — a nested inner form gets its tag dropped and
//!   closes the outer form early. The `<img>` follows a finished upload at
//!   once.
//! - A reactive rename leaves the *URL* on the old slug (the javascript-off
//!   path 303s to the new one). Saving again, deleting and restoring all
//!   follow the new slug, but reloading the old URL gets the not-found page.
//! - The list's "Tracks" column is a static `0` (the list service carries no
//!   track counts), and the editor's sidebar renders the real track list
//!   from the same `get_release` fetch.
//!
//! The page gate is the `label_owner` permission; the release services also
//! require `admin` (a `label_owner` without it gets the `forbidden` alert
//! on submit).

use dioxus::prelude::*;
use dioxus::router::RouterContext;

use crate::api_types::edit::EditOutcome;
use crate::api_types::release::ReleaseResult;
use crate::config::upload::UploadConfiguration;
use crate::forms::release::{ReleaseCreateFields, ReleaseSlugFields, ReleaseUpdateFields};
use crate::models::artist::Artist;
use crate::models::release::Release;
use crate::models::track::Track;
use crate::ui::assets::display_image_url;
use crate::ui::data::{get_artist, get_label_artists, get_release, get_releases};
use crate::ui::page_data;
use crate::ui::reactive::{
    posted_fields, posted_fields_with_ids, refused_code, saved_row, transport_message,
};
use crate::ui::upload::{FileUploadWithProgress, stored_file_url};
use crate::ui::use_label;
use crate::ui::use_page;
use crate::ui::use_user;
use crate::ui::utils::{
    ErrorPage, InlineError, Loading, StatusBadge, Success, ToastArea, permission_allows,
};
use crate::ui::{Route, redirect};

// Server-only: the twins' bodies run in the server half of the `#[post]`
// macros, next to the same services the javascript-off handlers call.
#[cfg(feature = "server")]
use crate::server::forms::release_code;
#[cfg(feature = "server")]
use crate::services;

use super::pages::{DateField, MarkdownField};
use super::track::TrackList;

/// The list of an artist's releases.
#[component]
pub fn Releases(
    /// The artist slug from the URL.
    artist_slug: String,
) -> Element {
    rsx! {
        SuspenseBoundary {
            fallback: |_context: SuspenseContext| rsx! { Loading {} },
            // Keyed so switching artists remounts the body; the key only
            // counts on fragment children, hence the one-item loop (dioxus
            // ignores `key:` on static children — see use_page).
            for artist_slug in [artist_slug] {
                ReleasesBody {
                    key: "{artist_slug}",
                    artist_slug,
                }
            }
        }
    }
}

#[component]
fn ReleasesBody(artist_slug: String) -> Element {
    let session = use_user()?;
    let user = session().ok().flatten();
    if !permission_allows(user.as_ref(), "label_owner") {
        redirect(Route::Dashboard {
            ready: None,
            purged: None,
            skipped: None,
            bad: None,
        });
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

    // Staff visibility comes from the service; the shared "show deleted"
    // toggle decides about soft-deleted rows (read inside the closure so
    // flipping it re-runs this load).
    let show_deleted = use_context::<super::AdminPrefs>().show_deleted;
    let releases_slug = artist_slug.clone();
    let releases_resource =
        use_page(move || page_data(get_releases(releases_slug.clone(), show_deleted())))?;
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

/// One table row. The "Tracks" count is a static `0` because the list
/// service carries no counts.
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
            // Keyed so the create targets the right artist; the key only
            // counts on fragment children, hence the one-item loop (dioxus
            // ignores `key:` on static children — see use_page).
            for (artist_slug, bad) in [(artist_slug, bad)] {
                CreateReleaseBody {
                    key: "{artist_slug}",
                    artist_slug,
                    bad,
                }
            }
        }
    }
}

#[component]
fn CreateReleaseBody(artist_slug: String, bad: Option<String>) -> Element {
    let session = use_user()?;
    let user = session().ok().flatten();
    if !permission_allows(user.as_ref(), "label_owner") {
        redirect(Route::Dashboard {
            ready: None,
            purged: None,
            skipped: None,
            bad: None,
        });
        return rsx! { Loading {} };
    }

    // The page URL addresses the artist, so the form starts with it as the
    // primary/selected artist.
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

    // The release is stamped with the label's id.
    let label_id = match use_label()?() {
        Ok(result) => result.record_label.id,
        Err(error) => {
            tracing::error!("Error while getting label for release create: {error}");
            0
        }
    };

    // Reactive create: the twin answers the new row and the page
    // navigates to its editor, which is what the 303 does without a client
    // build. The router is looked up rather than hooked (`use_navigator` would
    // panic) because the routerless SSR markup tests render this page alone.
    let router = try_use_context::<RouterContext>();
    let prefs = use_context::<super::AdminPrefs>();
    let show_deleted = prefs.show_deleted;
    let mut create = use_action(move |fields: ReleaseCreateFields| {
        let include_deleted = show_deleted();
        async move {
            let answer = create_release(include_deleted, fields).await;
            if matches!(answer, Ok(EditOutcome::Saved(_))) {
                prefs.refresh_menu();
            }
            answer
        }
    });
    let new_release_artist = artist_slug.clone();
    use_effect(move || {
        // The action resolves only after the submit handler has returned, so
        // leaving the page happens in an effect (as on the auth pages).
        let (Some(router), Some(result)) = (router, saved_row(create.value())) else {
            return;
        };
        // The only kind of failure is an external URL, which a `Route` is not.
        let _failure = router.replace(Route::EditRelease {
            artist_slug: new_release_artist.clone(),
            release_slug: result.release.slug,
            updated: None,
            bad: None,
            uploaded: None,
            bad_upload: None,
        });
    });
    let create_message = refused_code(create.value())
        .and_then(|code| release_message(Some(code.as_str())).map(str::to_string))
        .or_else(|| transport_message(create.value()));

    rsx! {
        form {
            method: "post",
            action: "/admin/forms/release",
            onsubmit: move |evt| {
                evt.prevent_default();
                create.call(posted_fields_with_ids::<ReleaseCreateFields>(
                    &evt,
                    "artist_ids",
                ));
            },
            div { class: "grid gap-6",
                ToastArea {
                    if let Some(message) = release_message(bad.as_deref()) {
                        InlineError { message: message.to_string() }
                    }
                    if let Some(message) = create_message {
                        InlineError { message }
                    }
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
            // Keyed on the row's params: dioxus keeps this instance when
            // only params change (one release to the next), and a page
            // load re-runs only on signal changes, so switching rows has
            // to remount the body — see `crate::ui::use_page`. The key
            // only counts on fragment children, hence the one-item loop.
            for (
                artist_slug,
                release_slug,
                updated,
                bad,
                uploaded,
                bad_upload,
            ) in [(artist_slug, release_slug, updated, bad, uploaded, bad_upload)]
            {
                EditReleaseBody {
                    key: "{artist_slug}/{release_slug}",
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
        redirect(Route::Dashboard {
            ready: None,
            purged: None,
            skipped: None,
            bad: None,
        });
        return rsx! { Loading {} };
    }

    // The artist carries the header name and the hidden `label_id`.
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

    // The single release row stays openable even when deleted (so Restore
    // can appear); its track listing follows the shared "show deleted"
    // toggle.
    let prefs = use_context::<super::AdminPrefs>();
    let show_deleted = prefs.show_deleted;
    let release_artist_slug = artist_slug.clone();
    let release_slug_for_resource = release_slug.clone();
    let release_resource = use_page(move || {
        page_data(get_release(
            release_artist_slug.clone(),
            release_slug_for_resource.clone(),
            show_deleted(),
        ))
    })?;
    let fetched = match release_resource() {
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

    // Reactive saves: update, delete and restore all go through one
    // action, so "the last thing the user did" is a single answer to read. With
    // a client build the `onsubmit` handlers below prevent the post and call
    // the twins at the bottom of this module; the plain form posts stay as the
    // javascript-off path (and are the only path a browser without one has).
    let mut edit = use_action(move |request: ReleaseEdit| {
        // The Saved payload's track list should match what the page
        // currently shows, so the toggle is read when the action runs.
        let include_deleted = show_deleted();
        async move {
            let answer = apply_release_edit(request, include_deleted).await;
            if matches!(answer, Ok(EditOutcome::Saved(_))) {
                prefs.refresh_menu();
            }
            answer
        }
    });
    let saved = saved_row(edit.value());
    let just_saved = saved.is_some();
    // The row on screen follows the last save: a rename brings its new slug
    // (so the next save and the uploader both target that row), and a
    // soft-delete swaps Delete for Restore without a page load.
    let result = saved.unwrap_or(fetched);
    let reactive_message = refused_code(edit.value())
        .and_then(|code| release_message(Some(code.as_str())).map(str::to_string))
        .or_else(|| transport_message(edit.value()));

    let release = result.release;
    // Read the image URL and alt text before the form takes the row apart
    // field by field (the uploader's `<img>` needs both).
    let current_image_url = display_image_url(&release.primary_image_url());
    let image_alt = release.name.clone();
    // The sidebar track list shares this fetch.
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
                ToastArea {
                    Success {
                        message: format!("{} Updated!", release.name),
                        show: updated.is_some() || just_saved,
                    }
                    if let Some(message) = release_message(bad.as_deref()) {
                        InlineError { message: message.to_string() }
                    }
                    if let Some(message) = reactive_message {
                        // Same fixed-code table the `?bad=` redirect path uses.
                        InlineError { message }
                    }
                }

        form {
            method: "post",
            action: "/admin/forms/release/update",
            onsubmit: move |evt| {
                evt.prevent_default();
                let fields = posted_fields_with_ids::<ReleaseUpdateFields>(&evt, "artist_ids");
                edit.call(ReleaseEdit::Update(fields));
            },
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
        // The image uploader lives after the field form: it brings its own
        // `<form>` and forms may not nest (module docs).
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
            onsubmit: move |evt: FormEvent| {
                evt.prevent_default();
                edit.call(ReleaseEdit::Delete(posted_fields::<ReleaseSlugFields>(
                    &evt,
                )));
            },
        }
        RestoreForm {
            slug: current_slug.clone(),
            artist_slug: artist_slug.clone(),
            onsubmit: move |evt: FormEvent| {
                evt.prevent_default();
                edit.call(ReleaseEdit::Restore(posted_fields::<ReleaseSlugFields>(
                    &evt,
                )));
            },
        }
            }
            div { class: "w-1/3",
                // The sidebar is the real track list, fed by
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
fn DeleteForm(
    slug: String,
    artist_slug: String,
    /// Reactive path: a client build soft-deletes through the twin, not a post.
    onsubmit: EventHandler<FormEvent>,
) -> Element {
    rsx! {
        form {
            id: "release-delete",
            method: "post",
            action: "/admin/forms/release/delete",
            onsubmit: onsubmit,
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
fn RestoreForm(
    slug: String,
    artist_slug: String,
    /// Reactive path: a client build restores through the twin, not a post.
    onsubmit: EventHandler<FormEvent>,
) -> Element {
    rsx! {
        form {
            id: "release-restore",
            method: "post",
            action: "/admin/forms/release/restore",
            onsubmit: onsubmit,
            input { r#type: "hidden", name: "slug", value: slug }
            input { r#type: "hidden", name: "artist_slug", value: artist_slug }
            button { class: "hidden", "Restore" }
        }
    }
}

/// The artist pickers. Without JS the checkboxes themselves carry the
/// selection: every checked one posts an
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
    // `label_owner` picks from every artist on the label). Soft-deleted
    // artists only surface with the "show deleted" toggle on; leaving
    // them out of the picker keeps a deleted artist from being
    // (re)linked by a save.
    let show_deleted = use_context::<super::AdminPrefs>().show_deleted;
    let artists_resource = use_page(move || page_data(get_label_artists(show_deleted())))?;
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

/// One artist's checkbox row — the checkbox *is* the form field.
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

// ---------------------------------------------------------------------------
// Reactive twins of the `/admin/forms/release*` handlers
// ---------------------------------------------------------------------------
//
// Same services and same fixed codes as the javascript-off handlers in
// `crate::server::forms`; only the delivery differs. A client build has no page
// load to carry `?updated=1` / `?bad=<code>`, and a webview window cannot
// follow a 303 at all, so a refusal answers with the very same code as data and
// a success answers the page's own `ReleaseResult`:
//
// ```text
// POST /api/update_release  {"fields":{"slug":"a-release","name":"B",…}}
// 200 {"Saved":{"release":{"slug":"b",…},"artists":[…],"tracks":[…]}}
//
// POST /api/update_release  {"fields":{"slug":"a-release","name":"",…}}
// 200 {"Failed":{"code":"name_required"}}
// ```
//
// They live on `/api/*_release` rather than on the form handlers' `/admin/forms/…` paths: axum
// allows only one handler per method and path.

/// What the release editor asks its action to do.
///
/// All three saves share one action so the page has a single answer to read —
/// which of three action signals fired most recently is otherwise guesswork.
#[expect(
    clippy::large_enum_variant,
    reason = "the update form is the variant the editor reaches for most, and the action's argument moves straight into the twin, so boxing it to shrink the two slug variants by a few hundred bytes would only add a dereference"
)]
enum ReleaseEdit {
    Update(ReleaseUpdateFields),
    Delete(ReleaseSlugFields),
    Restore(ReleaseSlugFields),
}

/// Runs one release-editor action through its twin. On a client build these
/// calls reach the generated stubs, so this lives on both sides.
async fn apply_release_edit(
    request: ReleaseEdit,
    include_deleted: bool,
) -> core::result::Result<EditOutcome<ReleaseResult>, HttpError> {
    match request {
        ReleaseEdit::Update(fields) => update_release(include_deleted, fields).await,
        ReleaseEdit::Delete(target) => delete_release(include_deleted, target).await,
        ReleaseEdit::Restore(target) => restore_release(include_deleted, target).await,
    }
}

/// Creates a release, answering the new row so the create page can navigate to
/// its editor (what the 303 handler does with its `Location`).
///
/// # Errors
/// 401 when the session is gone, 500 with a retry message when the create could
/// not run at all. Refusals (validation, permissions) answer `Failed`.
#[post("/api/create_release?include_deleted",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser
)]
pub async fn create_release(
    include_deleted: bool,
    fields: ReleaseCreateFields,
) -> core::result::Result<EditOutcome<ReleaseResult>, HttpError> {
    let Some(session_user) = user.0.as_ref() else {
        // The page gate redirects guests before they reach this form; a
        // hand-crafted POST gets the steer the 303 handler would have given.
        return HttpError::unauthorized("Please log in to save.");
    };
    match services::release::create_release_service(
        &db.0,
        Some(session_user),
        fields.into_service_form(),
        include_deleted,
    )
    .await
    {
        Ok(result) => Ok(EditOutcome::Saved(result)),
        Err(error) => {
            tracing::warn!("release create twin rejected: {error}");
            Ok(EditOutcome::Failed {
                code: release_code(&error).to_string(),
            })
        }
    }
}

/// Saves a release.
///
/// A rename re-sluggifies, and the answer carries the row's *current* slug,
/// which is what the editor then keeps saving to (and what the image uploader
/// targets).
///
/// # Errors
/// 401 when the session is gone, 500 with a retry message when the save could
/// not run at all.
#[post("/api/update_release?include_deleted",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser
)]
pub async fn update_release(
    include_deleted: bool,
    fields: ReleaseUpdateFields,
) -> core::result::Result<EditOutcome<ReleaseResult>, HttpError> {
    let Some(session_user) = user.0.as_ref() else {
        return HttpError::unauthorized("Please log in to save.");
    };
    match services::release::update_release_service(
        &db.0,
        Some(session_user),
        fields.into_service_form(),
        include_deleted,
    )
    .await
    {
        Ok(result) => Ok(EditOutcome::Saved(result)),
        Err(error) => {
            tracing::warn!("release update twin rejected: {error}");
            Ok(EditOutcome::Failed {
                code: release_code(&error).to_string(),
            })
        }
    }
}

/// Soft-deletes a release; the row in the answer carries `deleted_at`, which is
/// what swaps the editor's Delete button for Restore.
///
/// # Errors
/// 401 when the session is gone, 500 with a retry message when the delete could
/// not run at all.
#[post("/api/delete_release?include_deleted",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser
)]
pub async fn delete_release(
    include_deleted: bool,
    target: ReleaseSlugFields,
) -> core::result::Result<EditOutcome<ReleaseResult>, HttpError> {
    let Some(session_user) = user.0.as_ref() else {
        return HttpError::unauthorized("Please log in to save.");
    };
    match services::release::delete_release_service(
        &db.0,
        Some(session_user),
        target.slug,
        include_deleted,
    )
    .await
    {
        Ok(result) => Ok(EditOutcome::Saved(result)),
        Err(error) => {
            tracing::warn!("release delete twin rejected: {error}");
            Ok(EditOutcome::Failed {
                code: release_code(&error).to_string(),
            })
        }
    }
}

/// Restores a soft-deleted release.
///
/// # Errors
/// 401 when the session is gone, 500 with a retry message when the restore
/// could not run at all.
#[post("/api/restore_release?include_deleted",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser
)]
pub async fn restore_release(
    include_deleted: bool,
    target: ReleaseSlugFields,
) -> core::result::Result<EditOutcome<ReleaseResult>, HttpError> {
    let Some(session_user) = user.0.as_ref() else {
        return HttpError::unauthorized("Please log in to save.");
    };
    match services::release::restore_release_service(
        &db.0,
        Some(session_user),
        target.slug,
        include_deleted,
    )
    .await
    {
        Ok(result) => Ok(EditOutcome::Saved(result)),
        Err(error) => {
            tracing::warn!("release restore twin rejected: {error}");
            Ok(EditOutcome::Failed {
                code: release_code(&error).to_string(),
            })
        }
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
