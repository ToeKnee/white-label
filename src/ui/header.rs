//! The shared navbar: the record-label home link on the left, the user menu
//! on the right.
//!
//! Dioxus port of the daisyUI structure from Leptos
//! `components::record_label::header`.

use dioxus::prelude::*;

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
            div { class: "navbar-end", UserMenu {} }
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

/// The user menu: an avatar dropdown once signed in, Register/Login links
/// for guests (the default `User` context is a guest until Phase 4 wires
/// the session check).
#[component]
pub fn UserMenu() -> Element {
    let user = use_user();

    if user.read().is_authenticated() {
        let username = user.read().username.clone();
        let is_admin = user.read().permissions.contains("admin");
        let avatar_url = user.read().avatar_url();
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
                                        to: Route::EditProfile {},
                                        class: "btn btn-ghost",
                                        "Profile"
                                    }
                                }
                                li {
                                    Link {
                                        to: Route::ChangePassword {},
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
                                        to: Route::Logout {},
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
    } else {
        rsx! {
            span {
                Link { to: Route::Register {}, class: "btn btn-ghost", "Register" }
                Link { to: Route::Login {}, class: "btn btn-ghost", "Login" }
            }
        }
    }
}
