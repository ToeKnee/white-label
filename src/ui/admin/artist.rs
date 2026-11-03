//! The artist profile editors (MIGRATION.md Phase 4 slice 5a), ported from
//! the Leptos `components::admin::artist` create/edit pair plus its
//! `DeleteArtist`/`RestoreArtist` dialogs.
//!
//! Both pages post to plain-HTML handlers in `crate::server::forms`:
//! `POST /admin/forms/artist` creates, `POST /admin/forms/artist/update`
//! updates, and `POST /admin/forms/artist/delete` / `.../restore`
//! soft-delete and restore. The handlers 303 with fixed query codes
//! (`?bad=<code>` on failure, `?updated=1` on success); `artist_message`
//! maps the codes to text mirroring the service's messages — the parameter
//! value is never echoed.
//!
//! As in the Leptos editors the page gate is the `label_owner` permission,
//! while the services additionally require `admin`; a `label_owner`
//! without `admin` sees the form and then gets the `forbidden` alert after
//! submitting (same behaviour as the original).
//!
//! Deviations follow `super::pages` (see its module docs): flat field
//! names, JS-free `DateField` semantics (submitted local time is UTC), and
//! `form`-associated Delete/Restore sibling forms instead of `<dialog>`s.

use dioxus::prelude::*;

use crate::models::artist::Artist;
use crate::ui::data::get_artist;
use crate::ui::use_label;
use crate::ui::use_user;
use crate::ui::utils::{InlineError, Loading, Success, permission_allows};
use crate::ui::{Route, redirect};

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
    let session = use_user().suspend()?;
    let user = session().ok().flatten();
    if !permission_allows(user.as_ref(), "label_owner") {
        redirect(Route::Dashboard {});
        return rsx! { Loading {} };
    }

    // The artist is stamped with the label's id (the Leptos effect that
    // seeded it into the form state).
    let label_id = match use_label().suspend()?() {
        Ok(result) => result.record_label.id,
        Err(error) => {
            tracing::error!("Error while getting label for artist create: {error}");
            0
        }
    };

    rsx! {
        form { method: "post", action: "/admin/forms/artist",
            div { class: "grid gap-6",
                if let Some(message) = artist_message(bad.as_deref()) {
                    InlineError { message: message.to_string() }
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
    let session = use_user().suspend()?;
    let user = session().ok().flatten();
    if !permission_allows(user.as_ref(), "label_owner") {
        redirect(Route::Dashboard {});
        return rsx! { Loading {} };
    }

    // Unlike the public twin, the editor must also load soft-deleted
    // artists (so the Restore button can appear); `get_artist_service`
    // answers by slug with no visibility filter.
    let slug_for_resource = artist_slug.clone();
    let artist_result = use_resource(move || get_artist(slug_for_resource.clone())).suspend()?;
    let artist = match artist_result() {
        Ok(result) => result.artist,
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

    rsx! {
        document::Title { "{artist.name} Profile" }
        h1 { "{artist.name} Profile" }
        Success {
            message: format!("{} Updated!", artist.name),
            show: updated.is_some(),
        }
        if let Some(message) = artist_message(bad.as_deref()) {
            InlineError { message: message.to_string() }
        }

        form { method: "post", action: "/admin/forms/artist/update",
            EditArtistFields { artist, slug: artist_slug.clone() }
        }
        // Sibling forms for the row buttons (a `<form>` may not nest); the
        // buttons below point here with their `form` attribute.
        DeleteForm { slug: artist_slug.clone() }
        RestoreForm { slug: artist_slug }
    }
}

/// The editor inputs inside the update form (the Leptos `Form` piece of
/// `artist/edit.rs`).
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
#[component]
fn DeleteForm(slug: String) -> Element {
    rsx! {
        form { id: "artist-delete", method: "post", action: "/admin/forms/artist/delete",
            input { r#type: "hidden", name: "slug", value: slug }
            // Keyboard-submit fallback; the visible trigger is the row
            // button with `form="artist-delete"`.
            button { class: "hidden", "Delete" }
        }
    }
}

/// The restore sibling form (the `RestoreArtist` dialog's JS-free twin).
#[component]
fn RestoreForm(slug: String) -> Element {
    rsx! {
        form { id: "artist-restore", method: "post", action: "/admin/forms/artist/restore",
            input { r#type: "hidden", name: "slug", value: slug }
            button { class: "hidden", "Restore" }
        }
    }
}

/// The on-screen message for the artist form's fixed error codes; unknown
/// codes and absent codes render nothing. Mirrors the service and model
/// validation messages (`crate::server::forms` picks the codes).
fn artist_message(code: Option<&str>) -> Option<&'static str> {
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
