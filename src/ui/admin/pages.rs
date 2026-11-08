//! The marketing-page editors.
//!
//! Both pages post to plain-HTML handlers in `crate::server::forms`:
//! `POST /admin/forms/page` creates, `POST /admin/forms/page/update`
//! updates, and `POST /admin/forms/page/delete` / `.../restore` soft-delete
//! and restore. The handlers 303 with fixed query codes (`?bad=<code>` on
//! failure, `?updated=1` on success); `page_message` maps the codes to text
//! mirroring the service's messages — the parameter value is never echoed.
//!
//! The editors also save **reactively**: with a client
//! build `onsubmit` prevents the post and one `use_action` calls the JSON
//! twins at the bottom of this module (`/api/*_page`), which answer the
//! page's own `PageResult` (or the same fixed codes as data). The row on screen
//! then follows the save — a rename carries its new slug into the next save and
//! to the Delete/Restore buttons, and a soft-delete swaps Delete for Restore —
//! with no page load, which is what makes these editors usable in a desktop
//! window (a 303 has nothing to redirect there). The plain form posts stay
//! underneath as the javascript-off path.
//!
//! Design notes:
//!
//! - Field names are flat (`name`, `body`, `slug`, `published_at`).
//! - `DateField` has no JS to keep the hidden full-date field in sync: the
//!   hidden field carries the saved value, the
//!   visible `datetime-local` is preferred by the handler when present,
//!   and a submitted local time is interpreted as UTC (the browser's own
//!   zone is not applied). The timezone hint shows the stored offset.
//! - The delete/restore confirm `<dialog>` needed JS; here the row carries
//!   a button whose `form` attribute points at a sibling `<form>` posting
//!   to the delete/restore handler, so the swap of Delete/Restore after a
//!   reload works without script. The confirm step is dropped.
//! - A reactive rename leaves the *URL* on the old slug (the javascript-off
//!   path 303s to the new one). Saving again, deleting and restoring all
//!   follow the new slug, but reloading the old URL gets the not-found page.

use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use dioxus::router::RouterContext;

use crate::api_types::edit::EditOutcome;
use crate::api_types::page::PageResult;
use crate::forms::SlugFields;
use crate::forms::page::{PageCreateFields, PageUpdateFields};
use crate::models::page::Page;
use crate::ui::data::get_page;
use crate::ui::markdown_html;
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
use crate::server::forms::page_code;
#[cfg(feature = "server")]
use crate::services;

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

    // The page is stamped with the label's id.
    let label_id = match use_label()?() {
        Ok(result) => result.record_label.id,
        Err(error) => {
            tracing::error!("Error while getting label for page create: {error}");
            0
        }
    };

    // Reactive create: the twin answers the new row and the page
    // navigates to its editor, which is what the 303 does without a client
    // build. The router is looked up rather than hooked (`use_navigator` would
    // panic) because the routerless SSR markup tests render this page alone.
    let router = try_use_context::<RouterContext>();
    let mut create =
        use_action(move |fields: PageCreateFields| async move { create_page(fields).await });
    use_effect(move || {
        // The action resolves only after the submit handler has returned, so
        // leaving the page happens in an effect (as on the auth pages).
        let (Some(router), Some(result)) = (router, saved_row(create.value())) else {
            return;
        };
        // The only kind of failure is an external URL, which a `Route` is not.
        let _failure = router.replace(Route::EditPage {
            slug: result.page.slug,
            updated: None,
            bad: None,
        });
    });
    let create_message = refused_code(create.value())
        .and_then(|code| page_message(Some(code.as_str())).map(str::to_string))
        .or_else(|| transport_message(create.value()));

    rsx! {
        form { method: "post", action: "/admin/forms/page",
            onsubmit: move |evt| {
                evt.prevent_default();
                create.call(posted_fields::<PageCreateFields>(&evt));
            },
            div { class: "grid gap-6",
                if let Some(message) = page_message(bad.as_deref()) {
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

    // `label_owner` sessions see hidden pages too (the data twin's rule).
    let slug_for_resource = slug.clone();
    let page_result = use_page(move || page_data(get_page(slug_for_resource.clone())))?;
    let fetched = match page_result() {
        Ok(result) => result,
        Err(error) => {
            tracing::error!("Error while getting page for editor (slug {slug}): {error}");
            // No admin index route exists yet, so the same target is the
            // app's NotFound page, rendered inline.
            redirect(Route::NotFound {
                segments: vec!["admin".into(), "pages".into()],
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
        use_action(move |request: PageEdit| async move { apply_page_edit(request).await });
    let saved = saved_row(edit.value());
    let just_saved = saved.is_some();
    // The row on screen follows the last save: a rename brings its new slug
    // (so the next save and the Delete/Restore buttons both target that row),
    // and a soft-delete swaps Delete for Restore without a page load.
    let result = saved.unwrap_or(fetched);
    let reactive_message = refused_code(edit.value())
        .and_then(|code| page_message(Some(code.as_str())).map(str::to_string))
        .or_else(|| transport_message(edit.value()));

    let page = result.page;
    let current_slug = page.slug.clone();

    rsx! {
        document::Title { "Edit {page.name}" }
        h1 { "Edit {page.name}" }
        Success {
            message: format!("{} Updated!", page.name),
            show: updated.is_some() || just_saved,
        }
        if let Some(message) = page_message(bad.as_deref()) {
            InlineError { message: message.to_string() }
        }
        if let Some(message) = reactive_message {
            // Same fixed-code table the `?bad=` redirect path uses.
            InlineError { message }
        }

        form { method: "post", action: "/admin/forms/page/update",
            onsubmit: move |evt| {
                evt.prevent_default();
                edit.call(PageEdit::Update(posted_fields::<PageUpdateFields>(&evt)));
            },
            EditPageFields { page, slug: current_slug.clone() }
        }
        // Sibling forms for the row buttons (a `<form>` may not nest); the
        // buttons below point here with their `form` attribute, and with a
        // client build their `onsubmit` routes the submit to the action.
        DeleteForm {
            slug: current_slug.clone(),
            onsubmit: move |evt: FormEvent| {
                evt.prevent_default();
                edit.call(PageEdit::Delete(posted_fields::<SlugFields>(&evt)));
            },
        }
        RestoreForm {
            slug: current_slug,
            onsubmit: move |evt: FormEvent| {
                evt.prevent_default();
                edit.call(PageEdit::Restore(posted_fields::<SlugFields>(&evt)));
            },
        }
    }
}

/// The editor inputs inside the update form.
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
fn DeleteForm(
    slug: String,
    /// Reactive path: a client build soft-deletes through the twin, not a post.
    onsubmit: EventHandler<FormEvent>,
) -> Element {
    rsx! {
        form { id: "page-delete", method: "post", action: "/admin/forms/page/delete",
            onsubmit: onsubmit,
            input { r#type: "hidden", name: "slug", value: slug }
            // Keyboard-submit fallback; the visible trigger is the row
            // button with `form="page-delete"`.
            button { class: "hidden", "Delete" }
        }
    }
}

/// The restore sibling form (the `RestorePage` dialog's JS-free twin).
#[component]
fn RestoreForm(
    slug: String,
    /// Reactive path: a client build restores through the twin, not a post.
    onsubmit: EventHandler<FormEvent>,
) -> Element {
    rsx! {
        form { id: "page-restore", method: "post", action: "/admin/forms/page/restore",
            onsubmit: onsubmit,
            input { r#type: "hidden", name: "slug", value: slug }
            button { class: "hidden", "Restore" }
        }
    }
}

/// Date/time editor used by every admin form.
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

/// Markdown textarea beside a preview. The preview tracks what is typed:
/// a reactive draft starts from the saved text and a `markdown_html` re-render
/// follows every keystroke. With javascript off (or before hydration) the
/// preview shows the saved text, as the SSR render left it.
#[component]
pub fn MarkdownField(
    /// The title of the field, also the textarea's placeholder.
    title: String,
    /// The field name for the textarea.
    field: String,
    /// The saved markdown text, and the draft's starting value.
    markdown_text: String,
) -> Element {
    // The live preview reads this; the textarea drives it. `markdown_text` is
    // both the SSR preview and the draft's initial value, so the hydrated and
    // un-hydrated pages look identical until the user types.
    let mut draft = use_signal(|| markdown_text.clone());
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
                        value: "{draft}",
                        oninput: move |event| draft.set(event.value()),
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
                    dangerous_inner_html: markdown_html(&draft())
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Reactive twins of the `/admin/forms/page*` handlers
// ---------------------------------------------------------------------------
//
// Same services and same fixed codes as the javascript-off handlers in
// `crate::server::forms`; only the delivery differs. A client build has no page
// load to carry `?updated=1` / `?bad=<code>`, and a webview window cannot
// follow a 303 at all, so a refusal answers with the very same code as data and
// a success answers the page's own `PageResult`:
//
// ```text
// POST /api/update_page  {"fields":{"slug":"terms","name":"Terms",…}}
// 200 {"Saved":{"page":{"slug":"terms-of-service",…}}}
//
// POST /api/update_page  {"fields":{"slug":"terms","name":"",…}}
// 200 {"Failed":{"code":"name_required"}}
// ```
//
// They live on `/api/*_page` rather than on the form handlers' `/admin/forms/…` paths: axum
// allows only one handler per method and path.

/// What the page editor asks its action to do.
///
/// Named `PageEdit` (not `EditPage`): the `#[component]` macro already puts
/// the `EditPage` page in the type namespace, where an enum would collide.
///
/// All three saves share one action so the page has a single answer to read —
/// which of three action signals fired most recently is otherwise guesswork.
enum PageEdit {
    Update(PageUpdateFields),
    Delete(SlugFields),
    Restore(SlugFields),
}

/// Runs one page-editor action through its twin. On a client build these
/// calls reach the generated stubs, so this lives on both sides.
async fn apply_page_edit(
    request: PageEdit,
) -> core::result::Result<EditOutcome<PageResult>, HttpError> {
    match request {
        PageEdit::Update(fields) => update_page(fields).await,
        PageEdit::Delete(target) => delete_page(target).await,
        PageEdit::Restore(target) => restore_page(target).await,
    }
}

/// Creates a page, answering the new row so the create page can navigate to
/// its editor (what the 303 handler does with its `Location`).
///
/// # Errors
/// 401 when the session is gone, 500 with a retry message when the create could
/// not run at all. Refusals (validation, permissions) answer `Failed`.
#[post("/api/create_page",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser
)]
pub async fn create_page(
    fields: PageCreateFields,
) -> core::result::Result<EditOutcome<PageResult>, HttpError> {
    let Some(session_user) = user.0.as_ref() else {
        // The page gate redirects guests before they reach this form; a
        // hand-crafted POST gets the steer the 303 handler would have given.
        return HttpError::unauthorized("Please log in to save.");
    };
    match services::page::create_page_service(&db.0, Some(session_user), fields.into_service_form())
        .await
    {
        Ok(result) => Ok(EditOutcome::Saved(result)),
        Err(error) => {
            tracing::warn!("page create twin rejected: {error}");
            Ok(EditOutcome::Failed {
                code: page_code(&error).to_string(),
            })
        }
    }
}

/// Saves a page.
///
/// A rename re-sluggifies, and the answer carries the row's *current* slug,
/// which is what the editor then keeps saving to.
///
/// # Errors
/// 401 when the session is gone, 500 with a retry message when the save could
/// not run at all.
#[post("/api/update_page",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser
)]
pub async fn update_page(
    fields: PageUpdateFields,
) -> core::result::Result<EditOutcome<PageResult>, HttpError> {
    let Some(session_user) = user.0.as_ref() else {
        return HttpError::unauthorized("Please log in to save.");
    };
    match services::page::update_page_service(&db.0, Some(session_user), fields.into_service_form())
        .await
    {
        Ok(result) => Ok(EditOutcome::Saved(result)),
        Err(error) => {
            tracing::warn!("page update twin rejected: {error}");
            Ok(EditOutcome::Failed {
                code: page_code(&error).to_string(),
            })
        }
    }
}

/// Soft-deletes a page; the row in the answer carries `deleted_at`, which is
/// what swaps the editor's Delete button for Restore.
///
/// # Errors
/// 401 when the session is gone, 500 with a retry message when the delete could
/// not run at all.
#[post("/api/delete_page",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser
)]
pub async fn delete_page(
    target: SlugFields,
) -> core::result::Result<EditOutcome<PageResult>, HttpError> {
    let Some(session_user) = user.0.as_ref() else {
        return HttpError::unauthorized("Please log in to save.");
    };
    match services::page::delete_page_service(&db.0, Some(session_user), target.slug).await {
        Ok(result) => Ok(EditOutcome::Saved(result)),
        Err(error) => {
            tracing::warn!("page delete twin rejected: {error}");
            Ok(EditOutcome::Failed {
                code: page_code(&error).to_string(),
            })
        }
    }
}

/// Restores a soft-deleted page.
///
/// # Errors
/// 401 when the session is gone, 500 with a retry message when the restore
/// could not run at all.
#[post("/api/restore_page",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser
)]
pub async fn restore_page(
    target: SlugFields,
) -> core::result::Result<EditOutcome<PageResult>, HttpError> {
    let Some(session_user) = user.0.as_ref() else {
        return HttpError::unauthorized("Please log in to save.");
    };
    match services::page::restore_page_service(&db.0, Some(session_user), target.slug).await {
        Ok(result) => Ok(EditOutcome::Saved(result)),
        Err(error) => {
            tracing::warn!("page restore twin rejected: {error}");
            Ok(EditOutcome::Failed {
                code: page_code(&error).to_string(),
            })
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
