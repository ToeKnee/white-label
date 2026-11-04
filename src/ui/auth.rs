//! The auth pages (MIGRATION.md Phase 4 slice 3), ported from the Leptos
//! `components::auth` group.
//!
//! Dioxus has no Leptos `ActionForm`, and the web client's wasm bundle
//! arrives only in the later client-target phases — so these pages submit
//! through plain HTML forms. The forms post to the axum handlers in
//! `crate::server::forms` (namespaced under `/auth`, so the SSR pages keep
//! their own GET paths): the handlers run the service call, start or end
//! the session, then answer a real 303 — to the home page on success, or
//! back to the form with a fixed code in the query string (`?bad=<code>`,
//! `?saved=1`, `?updated=1`, `?done=1`) on the other outcomes. The mapping
//! from code to the on-screen message lives here (`login_message` and
//! friends); the parameter value itself is never echoed, so a crafted URL
//! cannot inject alert text.
//!
//! With JS enabled the same flow works unchanged (no app JS is needed for
//! it); when the client builds land, `use_action` interception can layer
//! on top of these endpoints without changing them.
//!
//! The avatar upload widget from the Leptos profile page is deliberately
//! not here yet — uploads are Phase 5 (`FileUploadWithProgress` onto dioxus
//! `FileData`/`FileStream`).
//!
//! Example — a failed login round trip:
//!
//! ```text
//! POST /auth/login   username=guest&password=nope     (form body)
//! 303 -> /login?bad=password                          (session untouched)
//! GET  /login?bad=password                            → alert "Password does not match."
//! ```

use dioxus::prelude::*;

use crate::models::auth::User;
use crate::ui::Route;
use crate::ui::redirect;
use crate::ui::use_user;
use crate::ui::utils::{InlineError, Loading, Success};

/// The login page. `bad` carries the code from a failed attempt; the
/// daisy fieldset and icons are kept 1:1 with the Leptos original.
#[component]
pub fn Login(
    /// Error code from a failed login attempt.
    bad: Option<String>,
) -> Element {
    rsx! {
        article { class: "my-6 md:container md:mx-auto prose",
            h1 { "Login" }
            form { method: "post", action: "/auth/login",
                div { class: "grid gap-6",
                    if let Some(message) = login_message(bad.as_deref()) {
                        InlineError { message: message.to_string() }
                    }

                    fieldset { class: "flex flex-col gap-6 fieldset",
                        label { class: "flex gap-2 items-center input",
                            UserIcon {}
                            input {
                                r#type: "text",
                                class: "grow",
                                placeholder: "Username",
                                name: "username",
                            }
                        }

                        label { class: "flex gap-2 items-center input",
                            KeyIcon {}
                            input {
                                r#type: "password",
                                class: "grow",
                                placeholder: "Password",
                                name: "password",
                            }
                        }

                        label { class: "label", r#for: "remember",
                            input {
                                r#type: "checkbox",
                                checked: true,
                                class: "checkbox",
                                name: "remember",
                                id: "remember",
                            }
                            "Remember me"
                        }

                        button { class: "btn btn-primary", "Login" }
                    }
                }
            }
        }
    }
}

/// The register page; `bad` carries the code from a failed attempt.
///
/// A successful register logs the new user in and 303s home (as the
/// Leptos page did).
#[component]
pub fn Register(
    /// Error code from a failed registration attempt.
    bad: Option<String>,
) -> Element {
    rsx! {
        article { class: "my-6 md:container md:mx-auto prose",
            h1 { "Register" }
            form { method: "post", action: "/auth/register",
                div { class: "grid gap-6",
                    if let Some(message) = register_message(bad.as_deref()) {
                        InlineError { message: message.to_string() }
                    }

                    fieldset { class: "flex flex-col gap-6 fieldset",
                        label { class: "flex gap-2 items-center input",
                            UserIcon {}
                            input {
                                r#type: "text",
                                class: "grow",
                                placeholder: "Username",
                                name: "username",
                            }
                        }

                        label { class: "flex gap-2 items-center input",
                            MailIcon {}
                            input {
                                r#type: "email",
                                class: "grow",
                                placeholder: "Email",
                                name: "email",
                            }
                        }

                        label { class: "flex gap-2 items-center input",
                            KeyIcon {}
                            input {
                                r#type: "password",
                                class: "grow",
                                placeholder: "Password",
                                name: "password",
                            }
                        }

                        label { class: "flex gap-2 items-center input",
                            KeyIcon {}
                            input {
                                r#type: "password",
                                class: "grow",
                                placeholder: "Password Confirmation",
                                name: "password_confirmation",
                            }
                        }

                        label { class: "label", r#for: "remember",
                            input {
                                r#type: "checkbox",
                                checked: true,
                                class: "checkbox",
                                name: "remember",
                                id: "remember",
                            }
                            "Remember me"
                        }

                        button { class: "btn btn-primary", "Register" }
                    }
                }
            }
        }
    }
}

/// The logout page. Before a logout it shows the button (posting ends the
/// session, then 303s back with `?done`).
///
/// After a logout it shows the Leptos page's "Logged out" line instead.
#[component]
pub fn Logout(
    /// Set when the session has just ended.
    done: Option<String>,
) -> Element {
    let finished = done.is_some();
    rsx! {
        article { class: "my-6 md:container md:mx-auto prose",
            h1 { "Log out" }
            div { class: "grid gap-6",
                if finished {
                    p { "Logged out" }
                } else {
                    form { method: "post", action: "/auth/logout",
                        button { class: "btn btn-primary", "Log out" }
                    }
                }
            }
        }
    }
}

/// The profile editor, guarded like the Leptos `EditProfile`
/// (`authenticated_or_redirect("/login")`).
#[component]
pub fn EditProfile(
    /// Set when the profile has just been saved.
    saved: Option<String>,
    /// Error code from a failed save.
    bad: Option<String>,
) -> Element {
    rsx! {
        document::Title { "Edit Profile" }
        article { class: "my-6 md:container md:mx-auto prose",
            h1 { "Edit Profile" }
            SuspenseBoundary {
                fallback: |_context: SuspenseContext| rsx! { Loading {} },
                ProfileForm { saved, bad }
            }
        }
    }
}

/// The profile form plus its alerts, resolved against the session user.
///
/// Guests hit the session guard below and the router redirects them to the
/// login page; the boundary's spinner only shows while the session
/// resource is in flight (never during the redirect itself).
#[component]
fn ProfileForm(
    /// Set when the profile has just been saved.
    saved: Option<String>,
    /// Error code from a failed save.
    bad: Option<String>,
) -> Element {
    let session = use_user()?;
    let user = match session() {
        Ok(Some(user)) if user.is_active() => user,
        Ok(_) => {
            redirect(Route::Login { bad: None });
            return rsx! { Loading {} };
        }
        Err(error) => {
            tracing::error!("Error while getting session user: {error}");
            redirect(Route::Login { bad: None });
            return rsx! { Loading {} };
        }
    };

    let success_message = format!("{} Updated!", user.username);
    rsx! {
        form { method: "post", action: "/auth/profile",
            div { class: "grid gap-6",
                Success { message: success_message, show: saved.is_some() }
                if let Some(message) = profile_message(bad.as_deref()) {
                    InlineError { message: message.to_string() }
                }
                ProfileFields { user }
            }
        }
    }
}

/// The profile inputs (the Leptos `Form` component), inside the page's
/// form element.
///
/// `original_username` names the account being edited and is cross-checked
/// against the session by the service.
#[component]
fn ProfileFields(user: User) -> Element {
    let original_username = user.username.clone();
    rsx! {
        input { r#type: "hidden", name: "original_username", value: original_username }

        div { class: "divider", "Public" }
        label { class: "flex gap-2 items-center input",
            input {
                r#type: "text",
                class: "grow",
                placeholder: "Username",
                name: "username",
                value: user.username,
            }
        }

        div { class: "divider", "Private" }
        label { class: "flex gap-2 items-center input",
            input {
                r#type: "email",
                class: "grow",
                placeholder: "Email",
                name: "email",
                value: user.email,
            }
        }

        label { class: "flex gap-2 items-center input",
            input {
                r#type: "text",
                class: "grow",
                placeholder: "First Name",
                name: "first_name",
                value: user.first_name.unwrap_or_default(),
            }
        }

        label { class: "flex gap-2 items-center input",
            input {
                r#type: "text",
                class: "grow",
                placeholder: "Last Name",
                name: "last_name",
                value: user.last_name.unwrap_or_default(),
            }
        }

        label { class: "flex gap-2 items-center input",
            input {
                r#type: "text",
                class: "grow",
                placeholder: "About me - tell us a little about yourself",
                name: "description",
                value: user.description.unwrap_or_default(),
            }
        }

        div { class: "flex flex-auto gap-6",
            button { class: "flex-1 btn btn-primary", "Update" }
        }
    }
}

/// The password change page, guarded like the Leptos `ChangePassword`.
#[component]
pub fn ChangePassword(
    /// Set when the password has just been changed.
    updated: Option<String>,
    /// Error code from a failed change.
    bad: Option<String>,
) -> Element {
    rsx! {
        document::Title { "Change Password" }
        article { class: "my-6 md:container md:mx-auto prose",
            h1 { "Change Password" }
            SuspenseBoundary {
                fallback: |_context: SuspenseContext| rsx! { Loading {} },
                PasswordForm { updated, bad }
            }
        }
    }
}

/// The password-change form plus its alerts, guarded on the session user.
#[component]
fn PasswordForm(
    /// Set when the password has just been changed.
    updated: Option<String>,
    /// Error code from a failed change.
    bad: Option<String>,
) -> Element {
    let session = use_user()?;
    match session() {
        Ok(Some(user)) if user.is_active() => {}
        Ok(_) => {
            redirect(Route::Login { bad: None });
            return rsx! { Loading {} };
        }
        Err(error) => {
            tracing::error!("Error while getting session user: {error}");
            redirect(Route::Login { bad: None });
            return rsx! { Loading {} };
        }
    }

    rsx! {
        form { method: "post", action: "/auth/change-password",
            div { class: "grid gap-6",
                Success {
                    message: "Password Updated!".to_string(),
                    show: updated.is_some(),
                }
                if let Some(message) = change_password_message(bad.as_deref()) {
                    InlineError { message: message.to_string() }
                }

                label { class: "flex gap-2 items-center input",
                    KeyIcon {}
                    input { r#type: "password", class: "grow", name: "password" }
                }

                label { class: "flex gap-2 items-center input",
                    KeyIcon {}
                    input { r#type: "password", class: "grow", name: "new_password" }
                }

                label { class: "flex gap-2 items-center input",
                    KeyIcon {}
                    input {
                        r#type: "password",
                        class: "grow",
                        name: "new_password_confirmation",
                    }
                }

                div { class: "flex flex-auto gap-6",
                    button { class: "flex-1 btn btn-primary", "Change Password" }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Query code -> alert message
// ---------------------------------------------------------------------------
//
// `crate::server::forms` translates each failure into one of these fixed
// codes (logging the service's own detail), and this module decides what
// the user reads. Unknown codes fall back to a generic line so an alert
// still appears instead of the form silently doing nothing.

/// Message for the login form's `?bad=` code.
fn login_message(code: Option<&str>) -> Option<&'static str> {
    match code {
        Some("required") => Some("Username and password are required."),
        Some("not_found") => Some("User does not exist."),
        Some("password") => Some("Password does not match."),
        Some(_) => Some("Login failed, try again later."),
        None => None,
    }
}

/// Message for the register form's `?bad=` code.
fn register_message(code: Option<&str>) -> Option<&'static str> {
    match code {
        Some("required") => Some("Username and password are required."),
        Some("mismatch") => Some("Passwords did not match."),
        Some("taken") => Some("Username or Email already taken."),
        Some("invalid") => Some("Email must be valid."),
        Some(_) => Some("Registration failed. Check the details and try again."),
        None => None,
    }
}

/// Message for the profile form's `?bad=` code.
fn profile_message(code: Option<&str>) -> Option<&'static str> {
    match code {
        Some("forbidden") => Some("You cannot update this account."),
        Some("taken") => Some("Username or Email already taken."),
        Some(_) => Some("Profile update failed, try again later."),
        None => None,
    }
}

/// Message for the change-password form's `?bad=` code.
fn change_password_message(code: Option<&str>) -> Option<&'static str> {
    match code {
        Some("mismatch") => Some("Passwords did not match."),
        Some("current") => Some("Password does not match."),
        Some(_) => Some("Password change failed, try again later."),
        None => None,
    }
}

// The daisyUI input icons, repeated across the Leptos originals' fields.

/// The person icon (username fields).
#[component]
fn UserIcon() -> Element {
    rsx! {
        svg {
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 16 16",
            fill: "currentColor",
            class: "w-4 h-4 opacity-70",
            path { d: "M8 8a3 3 0 1 0 0-6 3 3 0 0 0 0 6ZM12.735 14c.618 0 1.093-.561.872-1.139a6.002 6.002 0 0 0-11.215 0c-.22.578.254 1.139.872 1.139h9.47Z" }
        }
    }
}

/// The key icon (password fields).
#[component]
fn KeyIcon() -> Element {
    rsx! {
        svg {
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 16 16",
            fill: "currentColor",
            class: "w-4 h-4 opacity-70",
            path {
                fill_rule: "evenodd",
                d: "M14 6a4 4 0 0 1-4.899 3.899l-1.955 1.955a.5.5 0 0 1-.353.146H5v1.5a.5.5 0 0 1-.5.5h-2a.5.5 0 0 1-.5-.5v-2.293a.5.5 0 0 1 .146-.353l3.955-3.955A4 4 0 1 1 14 6Zm-4-2a.75.75 0 0 0 0 1.5.5.5 0 0 1 .5.5.75.75 0 0 0 1.5 0 2 2 0 0 0-2-2Z",
                clip_rule: "evenodd",
            }
        }
    }
}

/// The envelope icon (the register page's email field).
#[component]
fn MailIcon() -> Element {
    rsx! {
        svg {
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 16 16",
            fill: "currentColor",
            class: "w-4 h-4 opacity-70",
            path { d: "M2.5 3A1.5 1.5 0 0 0 1 4.5v.793c.026.009.051.02.076.032L7.674 8.51c.206.1.446.1.652 0l6.598-3.185A.755.755 0 0 1 15 5.293V4.5A1.5 1.5 0 0 0 13.5 3h-11Z" }
            path { d: "M15 6.954 8.978 9.86a2.25 2.25 0 0 1-1.956 0L1 6.954V11.5A1.5 1.5 0 0 0 2.5 13h11a1.5 1.5 0 0 0 1.5-1.5V6.954Z" }
        }
    }
}

#[cfg(test)]
mod tests {
    use dioxus::prelude::*;
    use dioxus::ssr::render_element;

    use super::{
        Login, Logout, Register, change_password_message, login_message, profile_message,
        register_message,
    };

    #[test]
    fn login_messages_cover_the_known_codes() {
        assert_eq!(
            login_message(Some("required")),
            Some("Username and password are required.")
        );
        assert_eq!(
            login_message(Some("not_found")),
            Some("User does not exist.")
        );
        assert_eq!(
            login_message(Some("password")),
            Some("Password does not match.")
        );
        // An unknown code still shows something rather than nothing.
        assert_eq!(
            login_message(Some("mystery")),
            Some("Login failed, try again later.")
        );
        assert_eq!(login_message(None), None);
    }

    #[test]
    fn register_messages_cover_the_known_codes() {
        assert_eq!(
            register_message(Some("mismatch")),
            Some("Passwords did not match.")
        );
        assert_eq!(
            register_message(Some("taken")),
            Some("Username or Email already taken.")
        );
        assert_eq!(
            register_message(Some("invalid")),
            Some("Email must be valid.")
        );
        assert_eq!(
            register_message(Some("mystery")),
            Some("Registration failed. Check the details and try again.")
        );
        assert_eq!(register_message(None), None);
    }

    #[test]
    fn profile_and_password_messages() {
        assert_eq!(
            profile_message(Some("forbidden")),
            Some("You cannot update this account.")
        );
        assert_eq!(
            profile_message(Some("taken")),
            Some("Username or Email already taken.")
        );
        assert_eq!(
            profile_message(Some("mystery")),
            Some("Profile update failed, try again later.")
        );
        assert_eq!(profile_message(None), None);

        assert_eq!(
            change_password_message(Some("mismatch")),
            Some("Passwords did not match.")
        );
        assert_eq!(
            change_password_message(Some("current")),
            Some("Password does not match.")
        );
        assert_eq!(
            change_password_message(Some("mystery")),
            Some("Password change failed, try again later.")
        );
        assert_eq!(change_password_message(None), None);
    }

    #[test]
    fn login_page_renders_the_form_and_only_known_alert_text() {
        let page = render_element(rsx! {
            Login { bad: Some("password".to_string()) }
        });
        assert!(page.contains("action=\"/auth/login\""), "{page}");
        assert!(page.contains("method=\"post\""), "{page}");
        assert!(page.contains("name=\"username\""), "{page}");
        assert!(page.contains("Password does not match."), "{page}");
        assert!(page.contains("alert alert-error"), "{page}");

        // An unknown code renders the generic line, never the code itself.
        let crafted = render_element(rsx! {
            Login { bad: Some("click-here".to_string()) }
        });
        assert!(
            crafted.contains("Login failed, try again later."),
            "{crafted}"
        );
        assert!(!crafted.contains("click-here"), "{crafted}");

        let clean = render_element(rsx! { Login { bad: None } });
        assert!(!clean.contains("alert"), "{clean}");
        assert!(clean.contains("h1>Login<"), "{clean}");
    }

    #[test]
    fn register_page_posts_to_its_handler() {
        let page = render_element(rsx! {
            Register { bad: None }
        });
        assert!(page.contains("action=\"/auth/register\""), "{page}");
        assert!(page.contains("name=\"password_confirmation\""), "{page}");
    }

    #[test]
    fn logout_swaps_the_button_for_the_done_line() {
        let before = render_element(rsx! {
            Logout { done: None }
        });
        assert!(before.contains("action=\"/auth/logout\""), "{before}");
        assert!(!before.contains("Logged out"), "{before}");

        let after = render_element(rsx! {
            Logout { done: Some("1".to_string()) }
        });
        assert!(after.contains("Logged out"), "{after}");
        assert!(!after.contains("action=\"/auth/logout\""), "{after}");
    }
}
