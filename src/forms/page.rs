//! This module defines the data structures used for creating and updating pages in a web application.

use super::parse_form_date;

/// Create a new page form structure.
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct CreatePageForm {
    /// Name of the page.
    pub name: String,
    /// Description of the page.
    pub description: String,
    /// Body content of the page. May contain markdown.
    pub body: String,
    /// Record label ID associated with the page.
    pub label_id: i64,
    /// Date and time when the page was published.
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Update an existing page form structure.
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct UpdatePageForm {
    /// Slug of the page, which is a unique identifier.
    pub slug: String,
    /// Name of the page.
    pub name: String,
    /// Description of the page.
    pub description: String,
    /// Body content of the page. May contain markdown.
    pub body: String,
    /// Record label ID associated with the page.
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// The page create form's posted fields, exactly as the browser posts them.
/// The name is flat (`page_form[…]` nesting was a Leptos server-fn
/// convention this plain-form surface does not need).
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct PageCreateFields {
    /// The label the page is stamped with (a hidden field).
    pub label_id: String,
    /// The page's name; the server derives the slug from it.
    pub name: String,
    /// The page's description; may contain markdown.
    pub description: String,
    /// The page's body; may contain markdown.
    pub body: String,
    /// Hidden full-date field (the stored value).
    pub published_at: String,
    /// Visible `datetime-local` value, preferred when present.
    pub local_published_at: String,
}

impl PageCreateFields {
    /// The service form these posts turn into, shared by the javascript-off
    /// handler and the reactive twin.
    #[must_use]
    pub fn into_service_form(self) -> CreatePageForm {
        CreatePageForm {
            name: self.name,
            description: self.description,
            body: self.body,
            label_id: self.label_id.parse().unwrap_or_default(),
            published_at: parse_form_date(&self.local_published_at, &self.published_at),
        }
    }
}

/// The page update form's posted fields: the create fields plus the addressed
/// slug.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct PageUpdateFields {
    /// The slug the editor addresses.
    pub slug: String,
    /// The page's name; the server re-derives the slug from it.
    pub name: String,
    /// The page's description; may contain markdown.
    pub description: String,
    /// The page's body; may contain markdown.
    pub body: String,
    /// Hidden full-date field (the stored value).
    pub published_at: String,
    /// Visible `datetime-local` value, preferred when present.
    pub local_published_at: String,
}

impl PageUpdateFields {
    /// The service form these posts turn into.
    #[must_use]
    pub fn into_service_form(self) -> UpdatePageForm {
        UpdatePageForm {
            slug: self.slug,
            name: self.name,
            description: self.description,
            body: self.body,
            published_at: parse_form_date(&self.local_published_at, &self.published_at),
        }
    }
}
