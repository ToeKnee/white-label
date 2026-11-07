//! The artist profile editors.
//!
//! Both pages post to plain-HTML handlers in `crate::server::forms`:
//! `POST /admin/forms/artist` creates, `POST /admin/forms/artist/update`
//! updates, and `POST /admin/forms/artist/delete` / `.../restore`
//! soft-delete and restore. The handlers 303 with fixed query codes
//! (`?bad=<code>` on failure, `?updated=1` on success); `artist_message`
//! maps the codes to text mirroring the service's messages — the parameter
//! value is never echoed.
//!
//! The editors also save **reactively**: with a client
//! build `onsubmit` prevents the post and one `use_action` calls the JSON
//! twins at the bottom of this module (`/api/*_artist`), which answer
//! the page's own `ArtistResult` (or the same fixed codes as data). The row on
//! screen then follows the save — a rename carries its new slug into the next
//! save and to the Delete/Restore buttons, and a soft-delete swaps Delete for
//! Restore — with no page load, which is what makes these editors usable in a
//! desktop window (a 303 has nothing to redirect there). The plain form posts
//! stay underneath as the javascript-off path.
//!
//! The page gate is the `label_owner` permission,
//! while the services additionally require `admin`; a `label_owner`
//! without `admin` sees the form and then gets the `forbidden` alert after
//! submitting.
//!
//! The field and delete conventions follow `super::pages` (see its module
//! docs): flat field names, JS-free `DateField` semantics (submitted local
//! time is UTC), and `form`-associated Delete/Restore sibling forms instead
//! of `<dialog>`s.
//!
//! As in the release editors (`super::release`), a reactive rename leaves the
//! *URL* on the old slug (the javascript-off path 303s to the new one). Saving
//! again, deleting and restoring all follow the new slug, but reloading the old
//! URL gets the not-found page.

use dioxus::prelude::*;
use dioxus::router::RouterContext;

use crate::api_types::artist::ArtistResult;
use crate::api_types::edit::EditOutcome;
use crate::forms::SlugFields;
use crate::forms::artist::{ArtistCreateFields, ArtistUpdateFields};
use crate::models::artist::Artist;
use crate::ui::data::get_artist;
use crate::ui::page_data;
use crate::ui::reactive::{posted_fields, refused_code, saved_row, transport_message};
use crate::ui::use_label;
use crate::ui::use_page;
use crate::ui::use_user;
use crate::ui::utils::{InlineError, Loading, Success, permission_allows};
use crate::ui::{Route, redirect};

// Server-only: the twins' bodies run in the server half of the `#[post]`
// macros, next to the same services the javascript-off handlers call.
#[cfg(feature = "server")]
use crate::server::forms::artist_code;
#[cfg(feature = "server")]
use crate::services;

use super::pages::{DateField, MarkdownField};

/// Create an artist; `bad` carries the create handler's code.
#[component]
pub fn CreateArtist(
    /// Error code from a failed create.
    bad: Option<String>,
) -> Element {
    rsx! {
        document::Title { "Create Artist" }
        h1 { "Create Artist" }
        SuspenseBoundary {
            fallback: |_context: SuspenseContext| rsx! { Loading {} },
            CreateArtistBody { bad }
        }
    }
}

#[component]
fn CreateArtistBody(
    /// Error code from a failed create.
    bad: Option<String>,
) -> Element {
    let session = use_user()?;
    let user = session().ok().flatten();
    if !permission_allows(user.as_ref(), "label_owner") {
        redirect(Route::Dashboard {});
        return rsx! { Loading {} };
    }

    // The artist is stamped with the label's id.
    let label_id = match use_label()?() {
        Ok(result) => result.record_label.id,
        Err(error) => {
            tracing::error!("Error while getting label for artist create: {error}");
            0
        }
    };

    // Reactive create: the twin answers the new row and the page
    // navigates to its editor, which is what the 303 does without a client
    // build. The router is looked up rather than hooked (`use_navigator` would
    // panic) because the routerless SSR markup tests render this page alone.
    let router = try_use_context::<RouterContext>();
    let mut create =
        use_action(move |fields: ArtistCreateFields| async move { create_artist(fields).await });
    use_effect(move || {
        // The action resolves only after the submit handler has returned, so
        // leaving the page happens in an effect (as on the auth pages).
        let (Some(router), Some(result)) = (router, saved_row(create.value())) else {
            return;
        };
        // The only kind of failure is an external URL, which a `Route` is not.
        let _failure = router.replace(Route::EditArtist {
            artist_slug: result.artist.slug,
            updated: None,
            bad: None,
        });
    });
    let create_message = refused_code(create.value())
        .and_then(|code| artist_message(Some(code.as_str())).map(str::to_string))
        .or_else(|| transport_message(create.value()));

    rsx! {
        form { method: "post", action: "/admin/forms/artist",
            onsubmit: move |evt| {
                evt.prevent_default();
                create.call(posted_fields::<ArtistCreateFields>(&evt));
            },
            div { class: "grid gap-6",
                if let Some(message) = artist_message(bad.as_deref()) {
                    InlineError { message: message.to_string() }
                }
                if let Some(message) = create_message {
                    InlineError { message }
                }
                input { r#type: "hidden", name: "label_id", value: label_id.to_string() }
                div { class: "divider", "Public" }
                label { class: "flex gap-2 items-center input",
                    input {
                        r#type: "text",
                        class: "grow",
                        placeholder: "Artist name",
                        name: "name",
                    }
                }
                MarkdownField {
                    title: "Description".to_string(),
                    field: "description".to_string(),
                    markdown_text: String::new(),
                }
                label { class: "flex gap-2 items-center input",
                    input {
                        r#type: "text",
                        class: "grow",
                        placeholder: "Website",
                        name: "website",
                    }
                }
                div { class: "divider", "Private" }
                DateField {
                    title: "Published at".to_string(),
                    field: "published_at".to_string(),
                    date: None,
                }
                button { class: "btn btn-primary", "Create" }
            }
        }
    }
}

/// Edit an artist's profile; `updated` and `bad` carry the handler codes
/// from update/delete/restore redirects.
#[component]
pub fn EditArtist(
    /// The artist slug from the URL.
    artist_slug: String,
    /// Set when the artist has just been saved/deleted/restored.
    updated: Option<String>,
    /// Error code from a failed save/delete/restore.
    bad: Option<String>,
) -> Element {
    rsx! {
        SuspenseBoundary {
            fallback: |_context: SuspenseContext| rsx! { Loading {} },
            EditArtistBody {
                artist_slug,
                updated,
                bad,
            }
        }
    }
}

#[component]
fn EditArtistBody(artist_slug: String, updated: Option<String>, bad: Option<String>) -> Element {
    let session = use_user()?;
    let user = session().ok().flatten();
    if !permission_allows(user.as_ref(), "label_owner") {
        redirect(Route::Dashboard {});
        return rsx! { Loading {} };
    }

    // Unlike the public twin, the editor must also load soft-deleted
    // artists (so the Restore button can appear); `get_artist_service`
    // answers by slug with no visibility filter.
    let slug_for_resource = artist_slug.clone();
    let artist_result = use_page(move || page_data(get_artist(slug_for_resource.clone())))?;
    let fetched = match artist_result() {
        Ok(result) => result,
        Err(error) => {
            tracing::error!("Error while getting artist for editor (slug {artist_slug}): {error}");
            // Redirect to NotFound, but pick a URL that cannot re-match
            // this editor: a redirect round-trips through the route's
            // Display string, and "/admin/artist" alone re-parses as the
            // create page, while any other slug re-opens this editor and
            // loops. "artist-not-found" matches no route on purpose.
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
    let mut edit =
        use_action(move |request: ArtistEdit| async move { apply_artist_edit(request).await });
    let saved = saved_row(edit.value());
    let just_saved = saved.is_some();
    // The row on screen follows the last save: a rename brings its new slug
    // (so the next save and the Delete/Restore buttons both target that row),
    // and a soft-delete swaps Delete for Restore without a page load.
    let result = saved.unwrap_or(fetched);
    let reactive_message = refused_code(edit.value())
        .and_then(|code| artist_message(Some(code.as_str())).map(str::to_string))
        .or_else(|| transport_message(edit.value()));

    let artist = result.artist;
    let current_slug = artist.slug.clone();

    rsx! {
        document::Title { "{artist.name} Profile" }
        h1 { "{artist.name} Profile" }
        Success {
            message: format!("{} Updated!", artist.name),
            show: updated.is_some() || just_saved,
        }
        if let Some(message) = artist_message(bad.as_deref()) {
            InlineError { message: message.to_string() }
        }
        if let Some(message) = reactive_message {
            // Same fixed-code table the `?bad=` redirect path uses.
            InlineError { message }
        }

        form { method: "post", action: "/admin/forms/artist/update",
            onsubmit: move |evt| {
                evt.prevent_default();
                edit.call(ArtistEdit::Update(posted_fields::<ArtistUpdateFields>(&evt)));
            },
            EditArtistFields { artist, slug: current_slug.clone() }
        }
        // Sibling forms for the row buttons (a `<form>` may not nest); the
        // buttons below point here with their `form` attribute, and with a
        // client build their `onsubmit` routes the submit to the action.
        DeleteForm {
            slug: current_slug.clone(),
            onsubmit: move |evt: FormEvent| {
                evt.prevent_default();
                edit.call(ArtistEdit::Delete(posted_fields::<SlugFields>(&evt)));
            },
        }
        RestoreForm {
            slug: current_slug,
            onsubmit: move |evt: FormEvent| {
                evt.prevent_default();
                edit.call(ArtistEdit::Restore(posted_fields::<SlugFields>(&evt)));
            },
        }
    }
}

/// The editor inputs inside the update form.
#[component]
fn EditArtistFields(artist: Artist, slug: String) -> Element {
    rsx! {
        div { class: "grid gap-6",
            input { r#type: "hidden", name: "slug", value: slug }
            div { class: "divider", "Public" }
            label { class: "flex gap-2 items-center input",
                input {
                    r#type: "text",
                    class: "grow",
                    placeholder: "Artist name",
                    name: "name",
                    value: artist.name.clone(),
                }
            }
            MarkdownField {
                title: "Description".to_string(),
                field: "description".to_string(),
                markdown_text: artist.description.clone(),
            }
            label { class: "flex gap-2 items-center input",
                input {
                    r#type: "text",
                    class: "grow",
                    placeholder: "Website",
                    name: "website",
                    value: artist.website,
                }
            }
            div { class: "divider", "Private" }
            DateField {
                title: "Published at".to_string(),
                field: "published_at".to_string(),
                date: artist.published_at,
            }
            div { class: "flex flex-auto gap-6",
                button { class: "flex-1 btn btn-primary", "Update" }
                if artist.deleted_at.is_some() {
                    button {
                        class: "btn btn-secondary",
                        form: "artist-restore",
                        "Restore"
                    }
                } else {
                    button { class: "btn btn-error", form: "artist-delete", "Delete" }
                }
            }
        }
    }
}

/// The soft-delete sibling form (the `DeleteArtist` dialog's JS-free twin).
/// The soft-delete sibling form (shared with the links editor, which also
/// carries the artist delete/restore buttons).
#[component]
pub(super) fn DeleteForm(
    slug: String,
    /// Reactive path: a client build soft-deletes through the twin, not a post.
    onsubmit: EventHandler<FormEvent>,
) -> Element {
    rsx! {
        form { id: "artist-delete", method: "post", action: "/admin/forms/artist/delete",
            onsubmit: onsubmit,
            input { r#type: "hidden", name: "slug", value: slug }
            // Keyboard-submit fallback; the visible trigger is the row
            // button with `form="artist-delete"`.
            button { class: "hidden", "Delete" }
        }
    }
}

/// The restore sibling form (the `RestoreArtist` dialog's JS-free twin).
#[component]
pub(super) fn RestoreForm(
    slug: String,
    /// Reactive path: a client build restores through the twin, not a post.
    onsubmit: EventHandler<FormEvent>,
) -> Element {
    rsx! {
        form { id: "artist-restore", method: "post", action: "/admin/forms/artist/restore",
            onsubmit: onsubmit,
            input { r#type: "hidden", name: "slug", value: slug }
            button { class: "hidden", "Restore" }
        }
    }
}

/// The on-screen message for the artist form's fixed error codes; unknown
/// codes and absent codes render nothing. Mirrors the service and model
/// validation messages (`crate::server::forms` picks the codes; the links
/// editor maps its reactive artist failures with it too).
pub(super) fn artist_message(code: Option<&str>) -> Option<&'static str> {
    match code {
        Some("forbidden") => Some("You do not have permission."),
        Some("name_required") => Some("Name is required."),
        Some("name_length") => Some("Name must be less than 255 characters."),
        Some("slug_taken") => Some("Slug must be unique."),
        Some("no_label") => Some("Record Label does not exist."),
        Some("server") => Some("Something went wrong, please try again later."),
        Some(_) | None => None,
    }
}

// ---------------------------------------------------------------------------
// Reactive twins of the `/admin/forms/artist*` handlers
// ---------------------------------------------------------------------------
//
// Same services and same fixed codes as the javascript-off handlers in
// `crate::server::forms`; only the delivery differs. A client build has no page
// load to carry `?updated=1` / `?bad=<code>`, and a webview window cannot
// follow a 303 at all, so a refusal answers with the very same code as data and
// a success answers the page's own `ArtistResult`:
//
// ```text
// POST /api/update_artist  {"fields":{"slug":"jimi","name":"Jimi",…}}
// 200 {"Saved":{"artist":{"slug":"jimi-hendrix",…}}}
//
// POST /api/update_artist  {"fields":{"slug":"jimi","name":"",…}}
// 200 {"Failed":{"code":"name_required"}}
// ```
//
// They live on `/api/*_artist` rather than on the form handlers' `/admin/forms/…` paths: axum
// allows only one handler per method and path.

/// What the artist editor asks its action to do.
///
/// Named `ArtistEdit` (not `EditArtist`): the `#[component]` macro already
/// puts the `EditArtist` page in the type namespace, where an enum would
/// collide.
///
/// All three saves share one action so the page has a single answer to read —
/// which of three action signals fired most recently is otherwise guesswork.
enum ArtistEdit {
    Update(ArtistUpdateFields),
    Delete(SlugFields),
    Restore(SlugFields),
}

/// Runs one artist-editor action through its twin. On a client build these
/// calls reach the generated stubs, so this lives on both sides.
async fn apply_artist_edit(
    request: ArtistEdit,
) -> core::result::Result<EditOutcome<ArtistResult>, HttpError> {
    match request {
        ArtistEdit::Update(fields) => update_artist(fields).await,
        ArtistEdit::Delete(target) => delete_artist(target).await,
        ArtistEdit::Restore(target) => restore_artist(target).await,
    }
}

/// Creates an artist, answering the new row so the create page can navigate to
/// its editor (what the 303 handler does with its `Location`).
///
/// # Errors
/// 401 when the session is gone, 500 with a retry message when the create could
/// not run at all. Refusals (validation, permissions) answer `Failed`.
#[post("/api/create_artist",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser
)]
pub async fn create_artist(
    fields: ArtistCreateFields,
) -> core::result::Result<EditOutcome<ArtistResult>, HttpError> {
    let Some(session_user) = user.0.as_ref() else {
        // The page gate redirects guests before they reach this form; a
        // hand-crafted POST gets the steer the 303 handler would have given.
        return HttpError::unauthorized("Please log in to save.");
    };
    match services::artist::create_artist_service(
        &db.0,
        Some(session_user),
        fields.into_service_form(),
    )
    .await
    {
        Ok(result) => Ok(EditOutcome::Saved(result)),
        Err(error) => {
            tracing::warn!("artist create twin rejected: {error}");
            Ok(EditOutcome::Failed {
                code: artist_code(&error).to_string(),
            })
        }
    }
}

/// Saves an artist's profile.
///
/// A rename re-sluggifies, and the answer carries the row's *current* slug,
/// which is what the editor then keeps saving to.
///
/// # Errors
/// 401 when the session is gone, 500 with a retry message when the save could
/// not run at all.
#[post("/api/update_artist",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser
)]
pub async fn update_artist(
    fields: ArtistUpdateFields,
) -> core::result::Result<EditOutcome<ArtistResult>, HttpError> {
    let Some(session_user) = user.0.as_ref() else {
        return HttpError::unauthorized("Please log in to save.");
    };
    match services::artist::update_artist_service(
        &db.0,
        Some(session_user),
        fields.into_service_form(),
    )
    .await
    {
        Ok(result) => Ok(EditOutcome::Saved(result)),
        Err(error) => {
            tracing::warn!("artist update twin rejected: {error}");
            Ok(EditOutcome::Failed {
                code: artist_code(&error).to_string(),
            })
        }
    }
}

/// Soft-deletes an artist; the row in the answer carries `deleted_at`, which is
/// what swaps the editor's Delete button for Restore.
///
/// # Errors
/// 401 when the session is gone, 500 with a retry message when the delete could
/// not run at all.
#[post("/api/delete_artist",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser
)]
pub async fn delete_artist(
    target: SlugFields,
) -> core::result::Result<EditOutcome<ArtistResult>, HttpError> {
    let Some(session_user) = user.0.as_ref() else {
        return HttpError::unauthorized("Please log in to save.");
    };
    match services::artist::delete_artist_service(&db.0, Some(session_user), target.slug).await {
        Ok(result) => Ok(EditOutcome::Saved(result)),
        Err(error) => {
            tracing::warn!("artist delete twin rejected: {error}");
            Ok(EditOutcome::Failed {
                code: artist_code(&error).to_string(),
            })
        }
    }
}

/// Restores a soft-deleted artist.
///
/// # Errors
/// 401 when the session is gone, 500 with a retry message when the restore
/// could not run at all.
#[post("/api/restore_artist",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser
)]
pub async fn restore_artist(
    target: SlugFields,
) -> core::result::Result<EditOutcome<ArtistResult>, HttpError> {
    let Some(session_user) = user.0.as_ref() else {
        return HttpError::unauthorized("Please log in to save.");
    };
    match services::artist::restore_artist_service(&db.0, Some(session_user), target.slug).await {
        Ok(result) => Ok(EditOutcome::Saved(result)),
        Err(error) => {
            tracing::warn!("artist restore twin rejected: {error}");
            Ok(EditOutcome::Failed {
                code: artist_code(&error).to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::artist_message;

    #[test]
    fn artist_messages_cover_the_known_codes() {
        assert_eq!(
            artist_message(Some("forbidden")),
            Some("You do not have permission.")
        );
        assert_eq!(
            artist_message(Some("name_required")),
            Some("Name is required.")
        );
        assert_eq!(
            artist_message(Some("name_length")),
            Some("Name must be less than 255 characters.")
        );
        assert_eq!(
            artist_message(Some("slug_taken")),
            Some("Slug must be unique.")
        );
        assert_eq!(
            artist_message(Some("no_label")),
            Some("Record Label does not exist.")
        );
        assert_eq!(
            artist_message(Some("server")),
            Some("Something went wrong, please try again later.")
        );
    }

    #[test]
    fn unknown_and_missing_artist_codes_render_nothing() {
        assert_eq!(artist_message(Some("<script>alert(1)</script>")), None);
        assert_eq!(artist_message(None), None);
    }
}
