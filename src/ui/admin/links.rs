//! Edit an artist's music-service and social links (MIGRATION.md Phase 4
//! slice 5d), ported from the Leptos `components::admin::artist::links`.
//!
//! Like the other editors the page is gated on `label_owner`; the link
//! service additionally requires `admin`. Every platform always renders an
//! input (Leptos parity): a row for the artist shows its saved URL, an
//! unlinked platform an empty one. The service turns non-empty inputs into
//! create/update and empty inputs on saved rows into deletes.
//!
//! Deviations from the Leptos original:
//!
//! - The Leptos page updated reactively (its `ActionForm` never
//!   redirected). The port keeps that flow — `onsubmit` intercepts and a
//!   `use_action` calls the JSON twin `POST /api/update_links` (see
//!   [`crate::ui::data::update_links`]) — but keeps a plain
//!   `<form action="/admin/forms/links">` underneath, so the button still
//!   works before the wasm client lands (Phases 6-7): with no client the
//!   full POST 303s back with `?updated=1`/`?bad=` codes like the other
//!   editors.
//! - The success alert reads "… Links Updated!" (the Leptos copy said
//!   only "… Updated!").
//! - Leptos showed the raw server error text; the no-JS path shows fixed
//!   alerts from `links_message` instead (the reactive path keeps the raw
//!   message, matching the Leptos `ServerErrors` widget).

use convert_case::{Case, Casing};
use dioxus::prelude::*;

use crate::forms::links::LinksForm;
use crate::models::music_service::{MusicService, Platform};
use crate::models::social_media::{SocialMedia, SocialMediaService};
use crate::ui::data::{get_artist, get_links, update_links};
use crate::ui::use_user;
use crate::ui::utils::{InlineError, Loading, Success, permission_allows};
use crate::ui::{Route, redirect};

use super::artist::{DeleteForm, RestoreForm};

/// Edit an artist's external links (Leptos `artist/links.rs`).
#[component]
pub fn EditArtistLinks(
    /// The artist slug from the URL.
    artist_slug: String,
    /// Set when the links have just been saved.
    updated: Option<String>,
    /// Error code from a failed save.
    bad: Option<String>,
) -> Element {
    rsx! {
        SuspenseBoundary {
            fallback: |_context: SuspenseContext| rsx! { Loading {} },
            EditArtistLinksBody {
                artist_slug,
                updated,
                bad,
            }
        }
    }
}

#[component]
fn EditArtistLinksBody(
    artist_slug: String,
    updated: Option<String>,
    bad: Option<String>,
) -> Element {
    let session = use_user().suspend()?;
    let user = session().ok().flatten();
    if !permission_allows(user.as_ref(), "label_owner") {
        redirect(Route::Dashboard {});
        return rsx! { Loading {} };
    }

    // The header is "{name} Links" and the delete/restore buttons follow
    // the artist's soft-delete state, so the artist row is needed even
    // though the form itself only carries the slug (a missing artist gets
    // the NotFound page, as in the other editors).
    let header_artist_slug = artist_slug.clone();
    let artist = match use_resource(move || get_artist(header_artist_slug.clone())).suspend()?() {
        Ok(result) => result.artist,
        Err(error) => {
            tracing::error!(
                "Error while getting artist for the links editor (slug {artist_slug}): {error}"
            );
            redirect(Route::NotFound {
                segments: vec!["admin".into(), "artist-not-found".into()],
            });
            return rsx! { Loading {} };
        }
    };
    let artist_name = artist.name;
    let artist_deleted = artist.deleted_at.is_some();

    let links_artist_slug = artist_slug.clone();
    let fetched = use_resource(move || get_links(links_artist_slug.clone()));
    let resource_links = match fetched.suspend()?() {
        Ok(result) => result,
        Err(error) => {
            tracing::error!(
                "Error while getting links for the editor (slug {artist_slug}): {error}"
            );
            redirect(Route::NotFound {
                segments: vec!["admin".into(), "artist-not-found".into()],
            });
            return rsx! { Loading {} };
        }
    };

    // Reactive path: the client build calls the JSON twin and keeps the
    // user on the page (the redirect form action below is the no-JS
    // fallback). `saved` replaces the fetched rows once a submit succeeds.
    let mut submit = use_action(move |form: LinksForm| async move { update_links(form).await });
    let saved = submit.value();
    let action_succeeded = matches!(saved, Some(Ok(_)));
    let links = match saved.clone() {
        Some(Ok(signal)) => signal.cloned(),
        _ => resource_links,
    };

    // The form is a dynamic rsx node, so its attributes are evaluated
    // after the sibling Delete/Restore forms below; give the hidden input
    // its own copy to keep the borrow checker happy.
    let form_artist_slug = artist_slug.clone();

    rsx! {
        document::Title { "{artist_name} Links" }
        h1 { "{artist_name} Links" }
        Success {
            message: format!("{artist_name} Links Updated!"),
            show: updated.is_some() || action_succeeded,
        }
        if let Some(message) = links_message(bad.as_deref()) {
            InlineError { message: message.to_string() }
        }
        if let Some(Err(error)) = saved {
            // Reactive-path failures (the Leptos ServerErrors widget showed
            // the raw service message too).
            InlineError { message: error.to_string() }
        }

        form {
            method: "post",
            action: "/admin/forms/links",
            onsubmit: move |evt| {
                evt.prevent_default();
                if let Some(form) = links_form_from_values(&evt) {
                    submit.call(form);
                }
            },
            div { class: "grid gap-6",
                input { r#type: "hidden", name: "artist_slug", value: form_artist_slug }
                div { class: "divider", "Music Services" }
                MusicServiceCards { music_services: links.music_services }
                div { class: "divider", "Social Media" }
                SocialMediaCards { social_media_services: links.social_media_services }
                div { class: "flex flex-auto gap-6",
                    button { class: "flex-1 btn btn-primary", "Update" }
                    // Leptos parity: the links page also carries the
                    // artist delete/restore buttons.
                    if artist_deleted {
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
        DeleteForm { slug: artist_slug.clone() }
        RestoreForm { slug: artist_slug }
    }
}

/// Build a `LinksForm` from a form submit's text inputs. Dioxus' built-in
/// `parsed_values` deserializer sits behind a dioxus-html feature the dioxus
/// meta crate does not expose, so the values are mapped to JSON by hand.
/// Every links field is text (no file inputs), and the hidden `artist_slug`
/// carries the artist.
fn links_form_from_values(evt: &FormEvent) -> Option<LinksForm> {
    let mut fields = serde_json::Map::new();
    for (name, value) in evt.values() {
        if let FormValue::Text(text) = value {
            fields.insert(name, serde_json::Value::String(text));
        }
    }
    serde_json::from_value(serde_json::Value::Object(fields)).ok()
}

/// The music-service cards (Leptos `music_service.rs`): one card per
/// platform, accent-styled with the saved URL when the artist has a row,
/// secondary-styled and empty otherwise.
#[component]
fn MusicServiceCards(
    /// The artist's saved music-service rows.
    music_services: Vec<MusicService>,
) -> Element {
    // (card class, legend class, icon file, platform name, field name,
    // saved URL) per platform; the field names match the `LinksForm`
    // snake_case fields the service reads.
    let rows = Platform::iterator().map(|platform| {
        let name = platform.to_string();
        let saved = music_services
            .iter()
            .find(|service| service.platform == *platform);
        (
            if saved.is_some() {
                "shadow-sm card bg-accent"
            } else {
                "shadow-sm card bg-secondary"
            },
            if saved.is_some() {
                "fieldset-legend text-accent-content"
            } else {
                "fieldset-legend text-secondary-content"
            },
            format!("/images/music_services/{name}.svg"),
            name.to_case(Case::Title),
            name.to_case(Case::Snake),
            saved.map(|service| service.url.clone()).unwrap_or_default(),
        )
    });

    rsx! {
        div { class: "flex flex-wrap gap-4 justify-between",
            for (card_class, legend_class, icon, legend, field, url) in rows {
                div { class: "{card_class}",
                    div { class: "card-body",
                        figure {
                            img {
                                class: "w-auto h-12",
                                src: "{icon}",
                                alt: "{legend}",
                            }
                        }
                        fieldset { class: "fieldset",
                            legend { class: "{legend_class}", "{legend}" }
                            input {
                                r#type: "text",
                                class: "w-full input",
                                placeholder: "https://",
                                name: "{field}",
                                value: "{url}",
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The social-media cards (Leptos `social_media.rs`); same shape as
/// [`MusicServiceCards`] with the social platform set.
#[component]
fn SocialMediaCards(
    /// The artist's saved social-media rows.
    social_media_services: Vec<SocialMediaService>,
) -> Element {
    let rows = SocialMedia::iterator().map(|platform| {
        let name = platform.to_string();
        let saved = social_media_services
            .iter()
            .find(|service| service.platform == *platform);
        (
            if saved.is_some() {
                "shadow-sm card bg-accent text-base-content"
            } else {
                "shadow-sm card bg-secondary text-base-content"
            },
            if saved.is_some() {
                "fieldset-legend text-accent-content"
            } else {
                "fieldset-legend text-secondary-content"
            },
            format!("/images/social_media_services/{name}.svg"),
            name.to_case(Case::Title),
            name.to_case(Case::Snake),
            saved.map(|service| service.url.clone()).unwrap_or_default(),
        )
    });

    rsx! {
        div { class: "flex flex-wrap gap-4 justify-between",
            for (card_class, legend_class, icon, legend, field, url) in rows {
                div { class: "{card_class}",
                    div { class: "card-body",
                        figure {
                            img {
                                class: "w-32 h-auto",
                                src: "{icon}",
                                alt: "{legend}",
                            }
                        }
                        fieldset { class: "fieldset",
                            legend { class: "{legend_class}", "{legend}" }
                            input {
                                r#type: "text",
                                class: "w-full input",
                                placeholder: "https://",
                                name: "{field}",
                                value: "{url}",
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Maps the fixed `?bad=` codes the links handler sets to alerts. The
/// service can only fail on permissions, a missing artist, or the
/// database, so the set is short.
fn links_message(code: Option<&str>) -> Option<&'static str> {
    match code {
        Some("forbidden") => Some("You do not have permission."),
        Some("no_artist") => Some("Artist does not exist."),
        Some("server") => Some("Something went wrong, please try again later."),
        Some(_) | None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::links_message;

    #[test]
    fn links_messages_cover_the_known_codes() {
        assert_eq!(
            links_message(Some("forbidden")),
            Some("You do not have permission.")
        );
        assert_eq!(
            links_message(Some("no_artist")),
            Some("Artist does not exist.")
        );
        assert_eq!(
            links_message(Some("server")),
            Some("Something went wrong, please try again later.")
        );
    }

    #[test]
    fn unknown_and_missing_links_codes_render_nothing() {
        assert_eq!(links_message(Some("<script>alert(1)</script>")), None);
        assert_eq!(links_message(None), None);
    }
}
