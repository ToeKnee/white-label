//! The record-label editor (MIGRATION.md Phase 4 slice 4), ported from the
//! Leptos `components::admin::edit_label` module.
//!
//! Like the slice-3 auth pages, the form posts to a plain-HTML handler in
//! `crate::server::forms` (`POST /admin/forms/label`), and the page reads
//! the handler's fixed query codes: `?updated=1` after a successful save,
//! `?bad=<code>` after a failure. Deviations from the Leptos editor:
//!
//! - The markdown preview is rendered server-side from the *saved*
//!   description; live preview while typing needs the client build.
//! - The success alert's name comes from the freshly loaded label (the
//!   redirect reload re-fetches it), where the original updated its store
//!   from the action result.

use dioxus::prelude::*;

use crate::ui::markdown_html;
use crate::ui::use_label;
use crate::ui::use_user;
use crate::ui::utils::{InlineError, Loading, Success, permission_allows};
use crate::ui::{Route, redirect};

/// The label editor, for `label_owner` sessions; `updated` and `bad` carry
/// the form handler's fixed codes.
#[component]
pub fn EditLabel(
    /// Set when the label has just been saved.
    updated: Option<String>,
    /// Error code from a failed save.
    bad: Option<String>,
) -> Element {
    rsx! {
        SuspenseBoundary {
            fallback: |_context: SuspenseContext| rsx! { Loading {} },
            EditLabelBody { updated, bad }
        }
    }
}

#[component]
fn EditLabelBody(
    /// Set when the label has just been saved.
    updated: Option<String>,
    /// Error code from a failed save.
    bad: Option<String>,
) -> Element {
    let session = use_user().suspend()?;
    let user = session().ok().flatten();
    // The Leptos guard redirected non-owners to `/admin`; guests get the
    // same steer (the handler enforces the real permission check).
    if !permission_allows(user.as_ref(), "label_owner") {
        redirect(Route::Dashboard {});
        return rsx! { Loading {} };
    }

    let label_result = use_label().suspend()?;
    let label = match label_result() {
        Ok(result) => result.record_label,
        Err(error) => {
            tracing::error!("Error while getting label for editor: {error}");
            crate::models::record_label::RecordLabel::default()
        }
    };

    rsx! {
        document::Title { "Edit {label.name}" }
        h1 { "Edit {label.name} Details" }
        form { method: "post", action: "/admin/forms/label",
            div { class: "grid gap-6",
                Success {
                    message: format!("{} Updated!", label.name),
                    show: updated.is_some(),
                }
                if let Some(message) = label_message(bad.as_deref()) {
                    InlineError { message: message.to_string() }
                }

                input {
                    r#type: "hidden",
                    name: "id",
                    value: label.id.to_string(),
                }
                div { class: "divider", "Public" }
                label { class: "flex gap-2 items-center input",
                    input {
                        r#type: "text",
                        class: "grow",
                        placeholder: "Label name",
                        name: "name",
                        value: label.name.clone(),
                    }
                }
                DescriptionFields { description: label.description.clone() }
                div { class: "divider", "Private" }
                IsrcDescription {}
                fieldset { class: "fieldset",
                    legend { class: "fieldset-legend", "ISRC Code prefix" }
                    input {
                        r#type: "text",
                        class: "w-full input",
                        placeholder: "ISRC prefix",
                        name: "isrc_base",
                        value: label.isrc_base,
                    }
                    p { class: "flex justify-between label",
                        span { "Country Code and First Registrant Code only" }
                        span { "Example {label.isrc_base} 25 00001" }
                    }
                }
                button { class: "btn btn-primary", "Update" }
            }
        }
    }
}

/// The description textarea beside a server-rendered markdown preview (the
/// Leptos `DescriptionFields`; its live preview needs the client build).
#[component]
fn DescriptionFields(
    /// The saved description, as the textarea's value and preview source.
    description: String,
) -> Element {
    rsx! {
        div { class: "flex gap-6",
            label { class: "w-1/2 fieldset",
                h2 { "Description" }
                textarea {
                    class: "w-full textarea",
                    rows: "15",
                    name: "description",
                    placeholder: "Description",
                    value: description.clone(),
                }
                p { class: "flex justify-between label",
                    span {}
                    span { "Markdown supported" }
                }
            }
            div { class: "w-1/2",
                h2 { "Preview" }
                div { dangerous_inner_html: markdown_html(&description) }
            }
        }
    }
}

/// The ISRC explainer, collapsed by CSS only (the Leptos
/// `ISRCDescription` text, kept verbatim).
#[component]
fn IsrcDescription() -> Element {
    rsx! {
        div { class: "collapse collapse-arrow bg-base-200",
            input { r#type: "checkbox" }
            div { class: "text-xl font-medium collapse-title", "What is an ISRC code?" }
            div { class: "collapse-content",
                p {
                    "An ISRC (International Standard Recording Code) is a unique 12-character alphanumeric identifier assigned to a specific recording of music. It serves as a standardized way to identify and track sound recordings across different territories, particularly within the digital marketplace for music distribution."
                }
                p {
                    "The ISRC system was created by IFPI (International Federation of the Phonographic Industry) in 1973 and is used worldwide for tracking the use of sound recordings. Each ISRC code represents a specific version or release of a particular track, which can be owned by different labels in different countries. It includes information about the recording company, title, and publisher, as well as details about when and where the recording was produced."
                }
                p { "ISRC codes are used primarily to:" }
                dl {
                    dt { "Track Usage" }
                    dd {
                        "Ensure that royalties are paid correctly for plays of a song on radio, TV, streaming services, or other media platforms across different countries."
                    }
                    dt { "Legal Purposes" }
                    dd {
                        "Support legal disputes related to copyright infringement by providing clear evidence of ownership and usage rights for a particular recording."
                    }
                    dt { "Sales and Marketing" }
                    dd {
                        "Facilitate the sale, licensing, or promotion of a music recording in various markets without needing to re-register it each time."
                    }
                    dt { "Technical Identification" }
                    dd {
                        "Aid in data management and metadata handling within digital music distribution platforms and databases."
                    }
                }
                p { "To obtain an ISRC code for a new recording:" }
                ol {
                    li {
                        "The record producer, publisher, or their authorized representative applies to a national registration body that is part of the IFPI system. This can be done before or after the release of the recording."
                    }
                    li {
                        "The application process involves providing detailed information about the recording, such as its title, artist(s), and creation date. A fee may also be charged for this service."
                    }
                    li {
                        "Once registered, an ISRC code is assigned to each distinct version of the recording that can be identified by these attributes. This includes different formats (e.g., CD, digital file) or releases in a multi-track release like an album."
                    }
                    li {
                        "The ISRC code should be included on all physical and digital copies of the recording as well as promotional materials to help identify it correctly."
                    }
                    li {
                        "Over time, if there are changes to the recording (such as new versions, edits, or remasters), these would need to be registered again with a new ISRC code if needed for legal or business purposes."
                    }
                }
                p {
                    "By using an ISRC code, music creators and distributors can streamline the process of managing rights and royalties across borders, making it easier to comply with international licensing agreements and regulations."
                }
            }
        }
    }
}

/// The on-screen message for the label form's fixed error codes; unknown
/// codes and absent codes render nothing. Mirrors the service's messages
/// (the code itself is never echoed); `crate::server::forms` picks the
/// codes from the service error.
fn label_message(code: Option<&str>) -> Option<&'static str> {
    match code {
        Some("forbidden") => Some("You do not have permission to update a label."),
        Some("server") => Some("Could not update label, try again later."),
        Some(_) | None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::label_message;

    #[test]
    fn messages_cover_the_known_codes_and_ignore_unknown_ones() {
        assert_eq!(
            label_message(Some("forbidden")),
            Some("You do not have permission to update a label.")
        );
        assert_eq!(
            label_message(Some("server")),
            Some("Could not update label, try again later.")
        );
        assert_eq!(label_message(Some("bogus")), None);
        assert_eq!(label_message(None), None);
    }
}
