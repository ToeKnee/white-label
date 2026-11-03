//! The shared navbar: the record-label home link on the left, the user menu
//! on the right.
//!
//! Dioxus port of the daisyUI structure from Leptos
//! `components::record_label::header`.

use dioxus::prelude::*;

use crate::models::auth::User;
use crate::ui::Route;
use crate::ui::use_label;
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
    let label = use_label().suspend()?;
    let name = match label() {
        Ok(result) => result.record_label.name,
        Err(_) => String::new(),
    };
    rsx! {
        Link { to: Route::RecordLabelHome {}, class: "text-xl btn btn-ghost", "{name}" }
    }
}

/// The user menu, resolved against the request's session: an avatar
/// dropdown once signed in, the guest links otherwise.
///
/// These are the two branches the Leptos `UserMenu` picked from its user
/// context.
#[component]
pub fn UserMenu() -> Element {
    let session = use_user().suspend()?;
    let user = session().ok().flatten();
    let Some(user) = user.filter(User::is_authenticated) else {
        return rsx! { GuestMenu {} };
    };

    let username = user.username.clone();
    let is_admin = user.permissions.contains("admin");
    let avatar_url = user.avatar_url();
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
                                    to: Route::EditProfile { saved: None, bad: None },
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
