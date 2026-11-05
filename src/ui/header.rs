//! The shared navbar: the record-label home link on the left, the user menu
//! on the right.
//!
//! Dioxus port of the daisyUI structure from Leptos
//! `components::record_label::header`.

use dioxus::prelude::*;

use crate::models::auth::User;
use crate::ui::Route;
use crate::ui::SessionHint;
use crate::ui::assets::display_image_url;
use crate::ui::use_label;
use crate::ui::use_session_hint;
use crate::ui::use_user;

/// Renders the page header.
#[component]
pub fn LabelHeader() -> Element {
    rsx! {
        div { class: "navbar bg-primary text-primary-content",
            div { class: "navbar-start",
                // The brand is the record label's name from the shared
                // label resource; the fallback keeps the navbar readable
                // on client builds before the fetch resolves.
                SuspenseBoundary {
                    fallback: |_context: SuspenseContext| rsx! {
                        Link { to: Route::RecordLabelHome {}, class: "text-xl btn btn-ghost", "White Label" }
                    },
                    BrandLink {}
                }
            }
            div {
                class: "navbar-end",
                // The menu comes from the request's own session (the
                // `GET /api/user` twin); the fallback shows the guest
                // links while a client build hasn't resolved it yet.
                SuspenseBoundary {
                    fallback: |_context: SuspenseContext| rsx! { GuestMenu {} },
                    UserMenu {}
                }
            }
        }
    }
}

/// The brand link: the record label's name (empty until a label exists,
/// as the Leptos store default left it with no label loaded).
#[component]
fn BrandLink() -> Element {
    let label = use_label()?;
    let name = match label() {
        Ok(result) => result.record_label.name,
        Err(_) => String::new(),
    };
    rsx! {
        Link { to: Route::RecordLabelHome {}, class: "text-xl btn btn-ghost", "{name}" }
    }
}

/// The navbar menu's account, picked from the page's session and whatever
/// reactive auth/profile events the client has since had.
///
/// A [`SessionHint::ProfileUpdated`] only swaps the avatar of the account it
/// names (the avatar upload renames the file to its uploader, so the username
/// already matches; the guard leaves any other session's avatar — and a guest
/// — exactly as the page render had them).
fn menu_user(hint: &SessionHint, page_session: Option<User>) -> Option<User> {
    match hint {
        // The twin's answer carries the whole user (permissions included), so
        // the menu can render the real thing without another round trip.
        // (Cloned out of the box: this match only borrows the hint.)
        SessionHint::SignedIn(user) => Some((**user).clone()),
        SessionHint::SignedOut => None,
        SessionHint::ProfileUpdated {
            username,
            avatar_file,
        } => page_session.map(|mut user| {
            if user.username == *username {
                user.avatar = Some(avatar_file.clone());
            }
            user
        }),
        // Nothing reactive has happened: show the session the server resolved
        // for this page (or, in a client build with no page load to carry it,
        // the one this component's own load fetched).
        SessionHint::Unknown => page_session,
    }
}

/// The user menu, resolved against the request's session: an avatar
/// dropdown once signed in, the guest links otherwise.
///
/// These are the two branches the Leptos `UserMenu` picked from its user
/// context. A sign-in or sign-out that never reloaded the page (the reactive
/// twins in [`crate::ui::auth`]) answers through the [`SessionHint`] context
/// instead: the chrome outlives client navigations, and a page resource
/// resolves once per component, so the menu needs the twin's own outcome to
/// catch up — and so it does for the profile page's avatar upload, which
/// answers through [`SessionHint::ProfileUpdated`] rather than reloading.
#[component]
pub fn UserMenu() -> Element {
    // Read the hint first so the hook order stays put when the session
    // resource below suspends the boundary and this component re-runs.
    let hint = use_session_hint();
    let session = use_user()?;
    let user = menu_user(&hint(), session().ok().flatten());
    let Some(user) = user.filter(User::is_authenticated) else {
        return rsx! { GuestMenu {} };
    };

    rsx! { SignedInMenu { user } }
}

/// The signed-in branch of the menu: the avatar dropdown with the profile,
/// password and (for admins) admin links, plus log out.
#[component]
fn SignedInMenu(user: User) -> Element {
    let username = user.username.clone();
    let is_admin = user.permissions.contains("admin");
    let avatar_url = display_image_url(&user.avatar_url());
    rsx! {
        div { class: "flex-none",
            ul { class: "px-1 menu menu-horizontal",
                li {
                    details {
                        summary {
                            div { class: "avatar",
                                div { class: "w-10 rounded-full",
                                    img {
                                        alt: format!("{}'s Avatar", username),
                                        src: avatar_url,
                                    }
                                }
                            }
                            "{username}"
                        }
                        ul { class: "p-2 rounded-t-none bg-base-100", "data-theme": "light",
                            li {
                                Link {
                                    to: Route::EditProfile {
                                        saved: None,
                                        bad: None,
                                        uploaded: None,
                                        bad_upload: None,
                                    },
                                    class: "btn btn-ghost",
                                    "Profile"
                                }
                            }
                            li {
                                Link {
                                    to: Route::ChangePassword { updated: None, bad: None },
                                    class: "btn btn-ghost",
                                    "Change Password"
                                }
                            }
                            if is_admin {
                                li {
                                    Link {
                                        to: Route::Dashboard {},
                                        class: "btn btn-ghost",
                                        "Admin"
                                    }
                                }
                            }
                            li {
                                Link {
                                    to: Route::Logout { done: None },
                                    class: "btn btn-ghost",
                                    "Log out"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The guest links (also the user menu's suspense fallback).
#[component]
fn GuestMenu() -> Element {
    rsx! {
        span {
            Link { to: Route::Register { bad: None }, class: "btn btn-ghost", "Register" }
            Link { to: Route::Login { bad: None }, class: "btn btn-ghost", "Login" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A signed-in account the menu decisions can be tested against.
    fn user(username: &str, avatar: Option<&str>) -> User {
        User {
            id: 7,
            username: username.to_string(),
            avatar: avatar.map(str::to_string),
            ..User::default()
        }
    }

    #[test]
    fn page_session_drives_the_menu_until_a_hint_arrives() {
        let signed_in = Some(user("jimi", Some("1-old.png")));
        assert_eq!(
            menu_user(&SessionHint::Unknown, signed_in.clone()),
            signed_in
        );
        assert_eq!(menu_user(&SessionHint::Unknown, None), None);
    }

    #[test]
    fn signed_in_and_signed_out_hints_replace_the_page_session() {
        let hint = SessionHint::SignedIn(Box::new(user("jimi", None)));
        let menu = menu_user(&hint, Some(user("someone-else", None)));
        assert!(
            menu.is_some_and(|menu| menu.username == "jimi"),
            "the twin's user wins"
        );

        assert_eq!(
            menu_user(&SessionHint::SignedOut, Some(user("jimi", None))),
            None
        );
    }

    #[test]
    fn a_profile_update_swaps_the_avatar_of_the_account_it_names() {
        let hint = SessionHint::ProfileUpdated {
            username: "jimi".to_string(),
            avatar_file: "42-new.png".to_string(),
        };
        let menu = menu_user(&hint, Some(user("jimi", Some("1-old.png")))).expect("menu user");
        assert_eq!(menu.avatar.as_deref(), Some("42-new.png"));
        // Everything else about the session user is untouched.
        assert_eq!(menu.username, "jimi");

        // A hint from another account (e.g. left over from an earlier session
        // in the same client) never renames this one's avatar.
        let other = user("other", Some("1-old.png"));
        assert_eq!(menu_user(&hint, Some(other.clone())), Some(other));

        // No session to update (guest) stays a guest.
        assert_eq!(menu_user(&hint, None), None);
    }
}
