//! Shared presentational components and auth guards.
//!
//! Ported from the Leptos `components::utils` group (MIGRATION.md Phase 4
//! slice 1). The daisyUI markup is kept 1:1 with the originals for
//! rendering similarity. Dioxus differences, per component group:
//!
//! - `ServerErrors` takes the app's own
//!   [`AppError`](crate::error::AppError), whose `Display` is already the
//!   bare message.
//! - The permission guards take the resolved session user and a [`Route`]
//!   instead of a URL string, and call the shared
//!   [`crate::ui::redirect`] helper.

use chrono::{DateTime, Utc};
use dioxus::prelude::*;

use crate::error::AppError;
use crate::models::auth::User;
use crate::ui::Route;
use crate::ui::redirect;

/// Wraps hero content in the `article > hero` shell the error/404 pages use.
fn hero(content: Element) -> Element {
    rsx! {
        article { class: "my-6 md:container md:mx-auto prose",
            div { class: "min-h-screen hero bg-base-200",
                div { class: "text-center hero-content",
                    div { class: "max-w-md",
                        {content}
                    }
                }
            }
        }
    }
}

/// A generic error page — the fallback an `ErrorBoundary` renders.
#[component]
pub fn ErrorPage() -> Element {
    hero(rsx! {
        h1 { class: "text-5xl font-bold", "Oh No..." }
        p { class: "py-6", "Something went wrong. Please try again later." }
    })
}

/// The 404 page for unmatched paths — the dioxus equivalent of the Leptos
/// `Routes fallback=NotFound`, with the daisy hero kept intact.
#[component]
pub fn NotFound(segments: Vec<String>) -> Element {
    // The catch-all swallows every unmatched path; make it visible to ops.
    tracing::debug!("no route matched: /{}", segments.join("/"));
    hero(rsx! {
        h1 { class: "text-5xl font-bold", "Oh No..." }
        p { class: "py-6", "Sorry, we couldn't find what you were looking for!" }
        p { class: "py-6", "Maybe it's hiding." }
    })
}

/// A loading spinner — the fallback inside `SuspenseBoundary`s.
#[component]
pub fn Loading() -> Element {
    rsx! {
        article { class: "my-6 md:container md:mx-auto prose",
            span { class: "loading loading-ring loading-lg" }
        }
    }
}

/// Inline error alert with the daisy error icon.
#[component]
pub fn InlineError(
    /// The message to display in the inline error.
    message: String,
) -> Element {
    // Keep a server-side breadcrumb: on-screen alerts are transient.
    tracing::error!("{message}");
    rsx! {
        div { role: "alert", class: "alert alert-error",
            svg {
                xmlns: "http://www.w3.org/2000/svg",
                class: "w-6 h-6 stroke-current shrink-0",
                fill: "none",
                view_box: "0 0 24 24",
                path {
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    stroke_width: "2",
                    d: "M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z",
                }
            }
            "{message}"
        }
    }
}

/// Show a failed server call's message as an inline error (nothing when the
/// call succeeded). `AppError`'s `Display` is the bare user-facing message,
/// so no variant-prefix trimming is needed like the Leptos wire format had.
#[component]
pub fn ServerErrors(
    /// The server error to display, if the call failed.
    server_errors: Option<AppError>,
) -> Element {
    server_errors.map_or_else(
        || rsx! {},
        |errors| {
            rsx! {
                InlineError { message: errors.to_string() }
            }
        },
    )
}

/// A success alert component.
#[component]
pub fn Success(
    /// The message to display in the alert.
    message: String,
    /// Whether to show the alert.
    show: bool,
) -> Element {
    if !show {
        return rsx! {};
    }
    rsx! {
        div { role: "alert", class: "alert alert-success",
            svg {
                xmlns: "http://www.w3.org/2000/svg",
                class: "w-6 h-6 stroke-current shrink-0",
                fill: "none",
                view_box: "0 0 24 24",
                path {
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    stroke_width: "2",
                    d: "M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z",
                }
            }
            span { "{message}" }
        }
    }
}

/// The badge (daisy class, text) for an item's state, or `None` when there
/// is nothing to show. Order of precedence: deleted beats a future publish
/// date ("Coming Soon"), and no publish date at all means "Draft". A past
/// publish date is simply live — no badge.
fn badge_for(
    deleted_at: Option<DateTime<Utc>>,
    published_at: Option<DateTime<Utc>>,
) -> Option<(&'static str, &'static str)> {
    if deleted_at.is_some() {
        Some(("indicator-item badge badge-error", "Deleted"))
    } else if published_at.is_some_and(|when| when > Utc::now()) {
        Some(("indicator-item badge badge-info", "Coming Soon"))
    } else if published_at.is_none() {
        Some(("indicator-item badge badge-warning", "Draft"))
    } else {
        None
    }
}

/// Shows the status of an item based on its deletion and publication dates.
#[component]
pub fn StatusBadge(
    /// The date when the item was deleted, if applicable.
    deleted_at: Option<DateTime<Utc>>,
    /// The date when the item was published, if applicable. This can also
    /// use the `release_date` field for releases.
    published_at: Option<DateTime<Utc>>,
) -> Element {
    badge_for(deleted_at, published_at).map_or_else(
        || rsx! {},
        |(badge_class, text)| {
            rsx! {
                span { class: badge_class, "{text}" }
            }
        },
    )
}

/// Guards a page: redirects to `to` unless the user is signed in. Does not
/// check permissions — see [`permission_or_redirect`].
///
/// Pass the user resolved from [`crate::ui::use_user`]; when this
/// redirects, the component should render [`Loading`] while the router
/// moves (the pattern `ui::auth`'s guarded pages use).
pub fn authenticated_or_redirect(user: Option<&User>, to: Route) {
    if !user.is_some_and(User::is_active) {
        redirect(to);
    }
}

/// Whether `user` passes the permission check — the condition behind
/// [`permission_or_redirect`], kept separate so page components can render
/// their loading state while the guard redirects.
///
/// The condition is kept intentionally identical to the Leptos original: an
/// *active* user passes even without the permission token, so this is closer
/// to "redirect guests" than a strict permission gate. Server-side handlers
/// and services enforce the real permission checks.
#[must_use]
pub fn permission_allows(user: Option<&User>, permission: &str) -> bool {
    user.is_some_and(|user| user.is_active() || user.permissions.contains(permission))
}

/// Guards a page: redirects to `to` unless the user passes the permission
/// check ([`permission_allows`]).
pub fn permission_or_redirect(user: Option<&User>, permission: &str, to: Route) {
    if !permission_allows(user, permission) {
        redirect(to);
    }
}

#[cfg(test)]
mod tests {
    use dioxus::prelude::*;
    use dioxus::ssr::render_element;

    use super::badge_for;
    use super::{ErrorPage, InlineError, Loading, NotFound, ServerErrors, StatusBadge, Success};
    use crate::error::AppError;

    /// The chrono dates as `None` need a type hint for the component props.
    const NO_DATE: Option<chrono::DateTime<chrono::Utc>> = None;

    fn badge(deleted: bool, published_days: Option<i64>) -> String {
        let deleted_at = deleted.then_some(chrono::Utc::now() - chrono::Duration::days(1));
        let published_at =
            published_days.map(|days| chrono::Utc::now() + chrono::Duration::days(days));
        render_element(rsx! {
            StatusBadge { deleted_at, published_at }
        })
    }

    #[test]
    fn badge_deleted_wins_and_is_error_styled() {
        let html = badge(true, None);
        assert!(html.contains("badge badge-error"), "{html}");
        assert!(html.contains("Deleted"), "{html}");

        // A deleted item keeps the Deleted badge even with a future date.
        let also_deleted = badge(true, Some(5));
        assert!(also_deleted.contains("Deleted"), "{also_deleted}");
        assert!(!also_deleted.contains("Coming Soon"), "{also_deleted}");
    }

    #[test]
    fn badge_future_publish_is_coming_soon() {
        let html = badge(false, Some(5));
        assert!(html.contains("badge badge-info"), "{html}");
        assert!(html.contains("Coming Soon"), "{html}");
    }

    #[test]
    fn badge_draft_when_never_published() {
        let html = badge(false, None);
        assert!(html.contains("badge badge-warning"), "{html}");
        assert!(html.contains("Draft"), "{html}");
    }

    #[test]
    fn badge_shows_nothing_for_a_past_publish_date() {
        let html = badge(false, Some(-5));
        assert!(!html.contains("badge"), "{html}");
        assert!(!html.contains("Draft"), "{html}");
    }

    #[test]
    fn badge_for_follows_the_status_precedence() {
        // The table behind the rendered tests above, including the boundary
        // semantics (`>` now is strict for Coming Soon).
        assert_eq!(
            badge_for(
                Some(chrono::Utc::now()),
                Some(chrono::Utc::now() + chrono::Duration::days(1))
            ),
            Some(("indicator-item badge badge-error", "Deleted"))
        );
        assert_eq!(
            badge_for(
                None,
                Some(chrono::Utc::now() + chrono::Duration::seconds(10))
            ),
            Some(("indicator-item badge badge-info", "Coming Soon"))
        );
        assert_eq!(
            badge_for(Some(chrono::Utc::now()), NO_DATE),
            Some(("indicator-item badge badge-error", "Deleted"))
        );
        assert_eq!(
            badge_for(None, NO_DATE),
            Some(("indicator-item badge badge-warning", "Draft"))
        );
        assert_eq!(
            badge_for(
                None,
                Some(chrono::Utc::now() - chrono::Duration::seconds(1))
            ),
            None
        );
    }

    #[test]
    fn success_renders_only_when_shown() {
        let shown = render_element(rsx! {
            Success {
                message: "Saved!".to_string(),
                show: true,
            }
        });
        assert!(shown.contains("alert alert-success"), "{shown}");
        assert!(shown.contains("Saved!"), "{shown}");

        let hidden = render_element(rsx! {
            Success {
                message: "Saved!".to_string(),
                show: false,
            }
        });
        assert!(!hidden.contains("alert"), "{hidden}");
    }

    #[test]
    fn inline_error_renders_the_message_in_an_alert() {
        let html = render_element(rsx! {
            InlineError {
                message: "Slug already taken.".to_string(),
            }
        });
        assert!(html.contains("alert alert-error"), "{html}");
        assert!(html.contains("Slug already taken."), "{html}");
        assert!(html.contains("role=\"alert\""), "{html}");
    }

    #[test]
    fn server_errors_shows_the_bare_message_or_nothing() {
        let failed = render_element(rsx! {
            ServerErrors {
                server_errors: Some(AppError::NotFound("Artist not found.".into())),
            }
        });
        // AppError's Display is the bare message, never a status-prefixed
        // string.
        assert!(failed.contains("Artist not found."), "{failed}");
        assert!(!failed.contains("NotFound|"), "{failed}");

        let succeeded = render_element(rsx! {
            ServerErrors { server_errors: None }
        });
        assert!(!succeeded.contains("alert"), "{succeeded}");
    }

    #[test]
    fn error_page_and_not_found_share_the_hero_shell() {
        let error = render_element(rsx! { ErrorPage {} });
        assert!(error.contains("hero bg-base-200"), "{error}");
        assert!(
            error.contains("Something went wrong. Please try again later."),
            "{error}"
        );

        let not_found = render_element(rsx! {
            NotFound {
                segments: vec!["nope".to_string(), "deep".to_string()],
            }
        });
        assert!(not_found.contains("hero bg-base-200"), "{not_found}");
        // Apostrophes come back HTML-escaped from the renderer.
        assert!(not_found.contains("Maybe it&#39;s hiding."), "{not_found}");
        assert!(not_found.contains("Oh No..."), "{not_found}");
    }

    #[test]
    fn loading_renders_the_daisy_ring() {
        let html = render_element(rsx! { Loading {} });
        assert!(html.contains("loading loading-ring loading-lg"), "{html}");
    }
}
