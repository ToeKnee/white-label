//! Edit an artist's music-service and social links.
//!
//! Like the other editors the page is gated on `label_owner`; the link
//! service additionally requires `admin`. Every platform always renders an
//! input: a row for the artist shows its saved URL, an
//! unlinked platform an empty one. The service turns non-empty inputs into
//! create/update and empty inputs on saved rows into deletes.
//!
//! The page's artist Delete/Restore buttons save
//! **reactively** too: with a client build the sibling forms' `onsubmit`
//! prevents the post and a `use_action` calls the artist twins in
//! `super::artist` (`/api/delete_artist` / `.../restore`), which answer
//! the artist row itself, so the header name and the Delete/Restore swap follow
//! the save with no page load. The plain sibling forms stay underneath as the
//! javascript-off path.
//!
//! Notes on the link-submit path itself:
//!
//! - It saves **reactively**: `onsubmit` intercepts and a
//!   `use_action` calls the JSON twin `POST /api/update_links` (see
//!   [`crate::ui::data::update_links`]) — but keeps a plain
//!   `<form action="/admin/forms/links">` underneath, so the button still
//!   works before the wasm client lands: with no client the
//!   full POST 303s back with `?updated=1`/`?bad=` codes like the other
//!   editors.
//! - The success alert reads "… Links Updated!".
//! - The no-JS path shows fixed alerts from `links_message` rather than raw
//!   server error text (the reactive path keeps the raw message in the
//!   `ServerErrors`-style widget).

use convert_case::{Case, Casing};
use dioxus::prelude::*;

use crate::api_types::artist::ArtistResult;
use crate::api_types::edit::EditOutcome;
use crate::forms::SlugFields;
use crate::forms::links::LinksForm;
use crate::models::music_service::{MusicService, Platform};
use crate::models::social_media::{SocialMedia, SocialMediaService};
use crate::ui::assets::bundled_asset;
use crate::ui::data::{get_artist, get_links, update_links};
use crate::ui::page_data;
use crate::ui::reactive::{posted_fields, refused_code, saved_row, transport_message};
use crate::ui::use_page;
use crate::ui::use_user;
use crate::ui::utils::{InlineError, Loading, Success, ToastArea, permission_allows};
use crate::ui::{Route, redirect};

use super::artist::{DeleteForm, RestoreForm, artist_message, delete_artist, restore_artist};

/// Edit an artist's external links.
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
            // Keyed so switching artists remounts the body; the key only
            // counts on fragment children, hence the one-item loop (dioxus
            // ignores `key:` on static children — see use_page).
            for (artist_slug, updated, bad) in [(artist_slug, updated, bad)] {
                EditArtistLinksBody {
                    key: "{artist_slug}",
                    artist_slug,
                    updated,
                    bad,
                }
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
    let session = use_user()?;
    let user = session().ok().flatten();
    if !permission_allows(user.as_ref(), "label_owner") {
        redirect(Route::Dashboard {
            ready: None,
            purged: None,
            skipped: None,
            bad: None,
            files: None,
        });
        return rsx! { Loading {} };
    }

    // The header is "{name} Links" and the delete/restore buttons follow
    // the artist's soft-delete state, so the artist row is needed even
    // though the form itself only carries the slug (a missing artist gets
    // the NotFound page, as in the other editors).
    let header_artist_slug = artist_slug.clone();
    let artist_resource = use_page(move || page_data(get_artist(header_artist_slug.clone())))?;
    let fetched_artist = match artist_resource() {
        Ok(result) => result,
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

    // Reactive artist delete/restore: the links form's row buttons
    // point at the sibling forms below; with a client build their `onsubmit`
    // calls the artist twins in `super::artist` instead of posting. The row
    // they answer replaces the fetched one, so the header name, the
    // Delete/Restore swap and the fixed-code alerts follow the save — the
    // links themselves keep their own action below.
    let prefs = use_context::<super::AdminPrefs>();
    let mut artist_row = use_action(move |request: ArtistRowEdit| async move {
        let answer = apply_artist_row_edit(request).await;
        if matches!(answer, Ok(EditOutcome::Saved(_))) {
            // A delete or restore changes what the menu lists.
            prefs.refresh_menu();
        }
        answer
    });
    let artist = saved_row(artist_row.value())
        .unwrap_or(fetched_artist)
        .artist;
    let artist_deleted = artist.deleted_at.is_some();
    let row_slug = artist.slug.clone();
    let artist_name = artist.name;
    let artist_row_message = refused_code(artist_row.value())
        .and_then(|code| artist_message(Some(code.as_str())).map(str::to_string))
        .or_else(|| transport_message(artist_row.value()));

    let links_artist_slug = artist_slug.clone();
    let fetched = use_page(move || page_data(get_links(links_artist_slug.clone())))?;
    let resource_links = match fetched() {
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

    // The links form addresses the slug from the URL: the reactive artist
    // save replaces the row on screen, not the page's address, and
    // the sibling forms below post `row_slug` instead.
    let form_artist_slug = artist_slug;

    rsx! {
        document::Title { "{artist_name} Links" }
        h1 { "{artist_name} Links" }
        ToastArea {
            Success {
                message: format!("{artist_name} Links Updated!"),
                show: updated.is_some() || action_succeeded,
            }
            if let Some(message) = links_message(bad.as_deref()) {
                InlineError { message: message.to_string() }
            }
            if let Some(Err(error)) = saved {
                // Reactive-path failures show the raw service message.
                InlineError { message: error.to_string() }
            }
            if let Some(message) = artist_row_message {
                // The artist twins refuse with the very codes the
                // `/admin/forms/artist/{delete,restore}` handlers put in `?bad=`.
                InlineError { message }
            }
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
                    // The links page also carries the artist
                    // delete/restore buttons.
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
        DeleteForm {
            slug: row_slug.clone(),
            onsubmit: move |evt: FormEvent| {
                evt.prevent_default();
                artist_row.call(ArtistRowEdit::Delete(posted_fields::<SlugFields>(&evt)));
            },
        }
        RestoreForm {
            slug: row_slug,
            onsubmit: move |evt: FormEvent| {
                evt.prevent_default();
                artist_row.call(ArtistRowEdit::Restore(posted_fields::<SlugFields>(&evt)));
            },
        }
    }
}

/// What the links page's artist row buttons ask the twins to do.
///
/// Delete and restore share one action so the page has a single answer to
/// read; the links save keeps its own action.
enum ArtistRowEdit {
    Delete(SlugFields),
    Restore(SlugFields),
}

/// Runs one links-page artist action through its twin. On a client build these
/// calls reach the generated stubs, so this lives on both sides.
async fn apply_artist_row_edit(
    request: ArtistRowEdit,
) -> core::result::Result<EditOutcome<ArtistResult>, HttpError> {
    match request {
        ArtistRowEdit::Delete(target) => delete_artist(target).await,
        ArtistRowEdit::Restore(target) => restore_artist(target).await,
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

/// The music-service cards: one card per
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
            bundled_asset(&format!("/images/music_services/{name}.svg")),
            name.to_case(Case::Title),
            name.to_case(Case::Snake),
            saved.map_or_default(|service| service.url.clone()),
        )
    });

    rsx! {
        div { class: "flex flex-wrap gap-4 justify-between",
            for (card_class, legend_class, icon, legend, field, url) in rows {
                div { class: "{card_class}",
                    div { class: "card-body",
                        figure {
                            // The card class stays the saved/unsaved signal, and `lofi`
                            // paints it dark, so the logo gets a light panel of its own here
                            // too: a plate around the logo alone would read as a black frame,
                            // while this panel spans the card. `bg-brand-plate` is the themed
                            // backing from `style/tailwind.css`; `ui::artist::MusicLinks`
                            // gives its logos the same backing.
                            class: "w-full rounded bg-brand-plate p-2",
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

/// The social-media cards; same shape as
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
            bundled_asset(&format!("/images/social_media_services/{name}.svg")),
            name.to_case(Case::Title),
            name.to_case(Case::Snake),
            saved.map_or_default(|service| service.url.clone()),
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
