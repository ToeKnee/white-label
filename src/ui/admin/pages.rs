//! The marketing-page editors (MIGRATION.md Phase 4 slice 4), ported from
//! the Leptos `components::admin::page` group plus the `admin::shared`
//! field components they use.
//!
//! Both pages post to plain-HTML handlers in `crate::server::forms`:
//! `POST /admin/forms/page` creates, `POST /admin/forms/page/update`
//! updates, and `POST /admin/forms/page/delete` / `.../restore` soft-delete
//! and restore. The handlers 303 with fixed query codes (`?bad=<code>` on
//! failure, `?updated=1` on success); `page_message` maps the codes to text
//! mirroring the service's messages — the parameter value is never echoed.
//!
//! Deviations from the Leptos editors:
//!
//! - Field names are flat (`name`, `body`, `slug`, `published_at`) instead
//!   of the Leptos `page_form[...]` struct-prefix encoding.
//! - `DateField` has no JS to keep the hidden full-date field in sync (no
//!   client build yet): the hidden field carries the saved value, the
//!   visible `datetime-local` is preferred by the handler when present,
//!   and a submitted local time is interpreted as UTC (the browser's own
//!   zone is not applied). The timezone hint shows the stored offset.
//! - The delete/restore confirm `<dialog>` needed JS; here the row carries
//!   a button whose `form` attribute points at a sibling `<form>` posting
//!   to the delete/restore handler, so the swap of Delete/Restore after a
//!   reload works without script. The confirm step is dropped.

use chrono::{DateTime, Utc};
use dioxus::prelude::*;

use crate::models::page::Page;
use crate::ui::data::get_page;
use crate::ui::markdown_html;
use crate::ui::use_label;
use crate::ui::use_user;
use crate::ui::utils::{InlineError, Loading, Success, permission_allows};
use crate::ui::{Route, redirect};

/// Create a marketing/legal page; `bad` carries the create handler's code.
#[component]
pub fn CreatePage(
    /// Error code from a failed create.
    bad: Option<String>,
) -> Element {
    rsx! {
        document::Title { "Create Page" }
        h1 { "Create Page" }
        SuspenseBoundary {
            fallback: |_context: SuspenseContext| rsx! { Loading {} },
            CreatePageBody { bad }
        }
    }
}

#[component]
fn CreatePageBody(
    /// Error code from a failed create.
    bad: Option<String>,
) -> Element {
    let session = use_user().suspend()?;
    let user = session().ok().flatten();
    if !permission_allows(user.as_ref(), "label_owner") {
        redirect(Route::Dashboard {});
        return rsx! { Loading {} };
    }

    // The page is stamped with the label's id (the Leptos effect that
    // seeded it into the form state).
    let label_id = match use_label().suspend()?() {
        Ok(result) => result.record_label.id,
        Err(error) => {
            tracing::error!("Error while getting label for page create: {error}");
            0
        }
    };

    rsx! {
        form { method: "post", action: "/admin/forms/page",
            div { class: "grid gap-6",
                if let Some(message) = page_message(bad.as_deref()) {
                    InlineError { message: message.to_string() }
                }
                input { r#type: "hidden", name: "label_id", value: label_id.to_string() }
                div { class: "divider", "Public" }
                label { class: "flex gap-2 items-center input",
                    input {
                        r#type: "text",
                        class: "grow",
                        placeholder: "Page name",
                        name: "name",
                    }
                }
                h2 { "Meta Description" }
                textarea {
                    class: "w-full textarea",
                    rows: "5",
                    name: "description",
                    placeholder: "Meta Description\nA short description of the page used for search engines.",
                }
                MarkdownField {
                    title: "Body".to_string(),
                    field: "body".to_string(),
                    markdown_text: String::new(),
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

/// Edit a marketing/legal page by slug; `updated` and `bad` carry the
/// handler codes from update/delete/restore redirects.
#[component]
pub fn EditPage(
    /// The page slug from the URL.
    slug: String,
    /// Set when the page has just been saved/deleted/restored.
    updated: Option<String>,
    /// Error code from a failed save/delete/restore.
    bad: Option<String>,
) -> Element {
    rsx! {
        SuspenseBoundary {
            fallback: |_context: SuspenseContext| rsx! { Loading {} },
            EditPageBody { slug, updated, bad }
        }
    }
}

#[component]
fn EditPageBody(slug: String, updated: Option<String>, bad: Option<String>) -> Element {
    let session = use_user().suspend()?;
    let user = session().ok().flatten();
    if !permission_allows(user.as_ref(), "label_owner") {
        redirect(Route::Dashboard {});
        return rsx! { Loading {} };
    }

    // `label_owner` sessions see hidden pages too (the data twin's rule).
    let slug_for_resource = slug.clone();
    let page_result = use_resource(move || get_page(slug_for_resource.clone())).suspend()?;
    let page = match page_result() {
        Ok(result) => result.page,
        Err(error) => {
            tracing::error!("Error while getting page for editor (slug {slug}): {error}");
            // The Leptos editor redirected to `/admin/pages`, which it also
            // had no route for — visitors landed on the 404 page. The
            // dioxus table has no admin index yet (slice 5), so the same
            // target is the app's NotFound page, rendered inline.
            redirect(Route::NotFound {
                segments: vec!["admin".into(), "pages".into()],
            });
            return rsx! { Loading {} };
        }
    };

    rsx! {
        document::Title { "Edit {page.name}" }
        h1 { "Edit {page.name}" }
        Success {
            message: format!("{} Updated!", page.name),
            show: updated.is_some(),
        }
        if let Some(message) = page_message(bad.as_deref()) {
            InlineError { message: message.to_string() }
        }

        form { method: "post", action: "/admin/forms/page/update",
            EditPageFields { page, slug: slug.clone() }
        }
        // Sibling forms for the row buttons (a `<form>` may not nest); the
        // buttons below point here with their `form` attribute.
        DeleteForm { slug: slug.clone() }
        RestoreForm { slug }
    }
}

/// The editor inputs inside the update form (the Leptos `Form` piece of
/// `edit.rs`).
#[component]
fn EditPageFields(page: Page, slug: String) -> Element {
    rsx! {
        div { class: "grid gap-6",
            input { r#type: "hidden", name: "slug", value: slug }
            div { class: "divider", "Public" }
            label { class: "flex gap-2 items-center input",
                input {
                    r#type: "text",
                    class: "grow",
                    placeholder: "Page name",
                    name: "name",
                    value: page.name.clone(),
                }
            }
            h2 { "Meta Description" }
            textarea {
                class: "w-full textarea",
                rows: "5",
                name: "description",
                placeholder: "Meta Description\nA short description of the page used for search engines.",
                value: page.description,
            }
            MarkdownField {
                title: "Body".to_string(),
                field: "body".to_string(),
                markdown_text: page.body.clone(),
            }
            div { class: "divider", "Private" }
            DateField {
                title: "Published at".to_string(),
                field: "published_at".to_string(),
                date: page.published_at,
            }
            div { class: "flex flex-auto gap-6",
                button { class: "flex-1 btn btn-primary", "Update" }
                if page.deleted_at.is_some() {
                    button {
                        class: "btn btn-secondary",
                        form: "page-restore",
                        "Restore"
                    }
                } else {
                    button { class: "btn btn-error", form: "page-delete", "Delete" }
                }
            }
        }
    }
}

/// The soft-delete sibling form (the `DeletePage` dialog's JS-free twin).
#[component]
fn DeleteForm(slug: String) -> Element {
    rsx! {
        form { id: "page-delete", method: "post", action: "/admin/forms/page/delete",
            input { r#type: "hidden", name: "slug", value: slug }
            // Keyboard-submit fallback; the visible trigger is the row
            // button with `form="page-delete"`.
            button { class: "hidden", "Delete" }
        }
    }
}

/// The restore sibling form (the `RestorePage` dialog's JS-free twin).
#[component]
fn RestoreForm(slug: String) -> Element {
    rsx! {
        form { id: "page-restore", method: "post", action: "/admin/forms/page/restore",
            input { r#type: "hidden", name: "slug", value: slug }
            button { class: "hidden", "Restore" }
        }
    }
}

/// Date/time editor used by every admin form (the Leptos `admin::shared`
/// `DateField`; slice 5 imports it for the artist/release/track editors).
///
/// The hidden full-date field carries the saved UTC value; the visible
/// `datetime-local` (named `local_<field>`) is preferred by the handler
/// when the browser sends one. See the module docs for the no-JS deviations.
#[component]
pub fn DateField(
    /// The title of the field, e.g. "Published at".
    title: String,
    /// The hidden field name, e.g. `published_at`.
    field: String,
    /// The stored date, if any.
    date: Option<DateTime<Utc>>,
) -> Element {
    let local_name = format!("local_{field}");
    let hidden_value = date.map(|when| when.to_string()).unwrap_or_default();
    let local_value = date
        .map(|when| when.format("%Y-%m-%dT%H:%M").to_string())
        .unwrap_or_default();
    let tz_hint =
        date.map(|when| format!("Timezone {} ({})", when.format("%z"), when.format("%Z")));

    rsx! {
        input { r#type: "hidden", name: field, value: hidden_value }
        fieldset { class: "w-full max-w-xs fieldset",
            label { class: "flex justify-between label", r#for: local_name.clone(),
                span { "{title}" }
                span { "*Date & Time" }
            }
            input {
                class: "w-full max-w-xs input",
                r#type: "datetime-local",
                id: local_name.clone(),
                name: local_name,
                value: local_value,
            }
            if let Some(tz_hint) = tz_hint {
                label { class: "flex justify-between label", r#for: local_name.clone(),
                    span {}
                    span { "{tz_hint}" }
                }
            }
        }
    }
}

/// Markdown textarea beside a server-rendered preview (the Leptos
/// `admin::shared` `MarkdownField`; slice 5 imports it). The live preview
/// while typing needs the client build.
#[component]
pub fn MarkdownField(
    /// The title of the field, also the textarea's placeholder.
    title: String,
    /// The field name for the textarea.
    field: String,
    /// The saved markdown text, previewed from the server.
    markdown_text: String,
) -> Element {
    rsx! {
        div { class: "flex gap-6",
            div { class: "w-1/2",
                h2 { "{title.clone()}" }
                fieldset { class: "fieldset",
                    textarea {
                        class: "w-full textarea",
                        rows: "15",
                        id: field.clone(),
                        name: field.clone(),
                        placeholder: title,
                        value: markdown_text.clone(),
                    }
                    label { class: "flex justify-between label", r#for: field,
                        span {}
                        span { "Markdown Supported" }
                    }
                }
            }
            div { class: "w-1/2",
                h2 { "Preview" }
                div { class: "w-full textarea",
                    dangerous_inner_html: markdown_html(&markdown_text)
                }
            }
        }
    }
}

/// The on-screen message for the page form's fixed error codes; unknown
/// codes and absent codes render nothing. Mirrors the service and model
/// validation messages (`crate::server::forms` picks the codes).
fn page_message(code: Option<&str>) -> Option<&'static str> {
    match code {
        Some("forbidden") => Some("You do not have permission."),
        Some("name_required") => Some("Name is required."),
        Some("name_length") => Some("Name must be less than 255 characters."),
        Some("slug_taken") => Some("Slug must be unique."),
        Some("desc_length") => Some("Description must be less than 255 characters."),
        Some("server") => Some("Something went wrong, please try again later."),
        Some(_) | None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::page_message;

    #[test]
    fn page_messages_cover_the_known_codes() {
        assert_eq!(
            page_message(Some("forbidden")),
            Some("You do not have permission.")
        );
        assert_eq!(
            page_message(Some("name_required")),
            Some("Name is required.")
        );
        assert_eq!(
            page_message(Some("name_length")),
            Some("Name must be less than 255 characters.")
        );
        assert_eq!(
            page_message(Some("slug_taken")),
            Some("Slug must be unique.")
        );
        assert_eq!(
            page_message(Some("desc_length")),
            Some("Description must be less than 255 characters.")
        );
        assert_eq!(
            page_message(Some("server")),
            Some("Something went wrong, please try again later.")
        );
    }

    #[test]
    fn unknown_and_missing_page_codes_render_nothing() {
        assert_eq!(page_message(Some("<script>alert(1)</script>")), None);
        assert_eq!(page_message(None), None);
    }
}
