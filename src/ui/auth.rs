//! The auth pages (MIGRATION.md Phase 4 slice 3), ported from the Leptos
//! `components::auth` group.
//!
//! Each page carries both submit paths, exactly like the upload widget.
//!
//! - The markup is a real `<form method="post" action="/auth/…">`, posting to
//!   the axum handlers in `crate::server::forms` (namespaced under `/auth`, so
//!   the SSR pages keep their own GET paths). Those handlers run the service
//!   call, start or end the session, then answer a real 303 — home on success,
//!   or back to the form with a fixed code in the query string (`?bad=<code>`,
//!   `?saved=1`, `?updated=1`, `?done=1`). That is the path the SSR-only
//!   server takes, and it needs no javascript.
//! - `onsubmit` prevents that post and calls the JSON twins at the bottom of
//!   this module (`/api/client/*`) through `use_action`, so the hydrated web
//!   client and the desktop window submit without a page load. For the window
//!   this is not an optimisation but the only path: a webview cannot follow a
//!   303, which is why it could not log in before slice 6d.
//!
//! Both paths fail identically. The twins answer with
//! [`crate::api_types::auth::AuthOutcome`], carrying the same fixed codes
//! `crate::server::forms` puts in the redirect, and the
//! tables at the bottom of this module (`login_message` and friends) decide
//! what the visitor reads; the parameter value itself is never echoed, so a
//! crafted URL cannot inject alert text.
//!
//! A reactive sign-in or sign-out also tells the shell's chrome what happened
//! through the [`crate::ui::SessionHint`] context: page components load the
//! session when they mount, but the shell's navbar outlives client
//! navigations, and a page resource resolves once per component, so the menu
//! needs the action's own answer rather than another fetch.
//!
//! The profile page also carries the avatar upload widget
//! ([`crate::ui::upload::FileUploadWithProgress`], Phase 5 slice 5f); like
//! the Leptos original it needs the client build, since its upload is a
//! request body rather than a form post.
//!
//! Example — a failed login round trip:
//!
//! ```text
//! POST /auth/login   username=guest&password=nope     (form body)
//! 303 -> /login?bad=password                          (session untouched)
//! GET  /login?bad=password                            → alert "Password does not match."
//! ```

use dioxus::CapturedError;
use dioxus::prelude::*;
use dioxus::router::RouterContext;

use crate::api_types::auth::AuthOutcome;
use crate::config::upload::UploadConfiguration;
use crate::forms::auth::{ClientLoginForm, ClientRegisterForm};
use crate::models::auth::User;
use crate::ui::Route;
use crate::ui::SessionHint;
use crate::ui::redirect;
use crate::ui::upload::FileUploadWithProgress;
use crate::ui::use_user;
use crate::ui::utils::{InlineError, Loading, Success};

// Server-only: the twins' bodies run in the server half of the macros, next to
// the same services and session records the form handlers use.
#[cfg(feature = "server")]
use crate::forms::user::RegisterUserForm;
#[cfg(feature = "server")]
use crate::server::current_user::{
    login as record_login, logout as record_logout, remember_session,
};
#[cfg(feature = "server")]
use crate::server::forms::{login_code, register_code};
#[cfg(feature = "server")]
use crate::services;

/// The login page. `bad` carries the code from a javascript-off attempt; a
/// reactive failure stays in the action instead. The daisy fieldset and icons
/// are kept 1:1 with the Leptos original.
#[component]
pub fn Login(
    /// Error code from a failed login attempt.
    bad: Option<String>,
) -> Element {
    // Looked up rather than hooked (`use_context`/`use_navigator` would panic)
    // because the SSR markup tests render this page on its own, with no app
    // shell and no router to provide either one.
    let router = try_use_context::<RouterContext>();
    let session_hint = try_use_context::<Signal<SessionHint>>();
    let mut submit =
        use_action(move |form: ClientLoginForm| async move { login_client(form).await });

    // Leaving the page happens in an effect: the action only resolves after the
    // submit handler has already returned.
    use_effect(move || {
        if let Some(AuthOutcome::LoggedIn { user }) = answered(submit.value()) {
            record_session_hint(session_hint, SessionHint::SignedIn(Box::new(user)));
            go_home(router);
        }
    });

    let answer = answered(submit.value());
    let reactive_message = match &answer {
        // Same fixed-code table the `?bad=` redirect path uses.
        Some(AuthOutcome::Failed { code }) => Some(
            login_message(Some(code.as_str()))
                .unwrap_or("Login failed, try again later.")
                .to_string(),
        ),
        _ => transport_message(submit.value()),
    };

    rsx! {
        article { class: "my-6 md:container md:mx-auto prose",
            h1 { "Login" }
            form {
                method: "post",
                action: "/auth/login",
                onsubmit: move |evt| {
                    evt.prevent_default();
                    submit.call(login_form_from_values(&evt));
                },
                div { class: "grid gap-6",
                    if let Some(message) = login_message(bad.as_deref()) {
                        InlineError { message: message.to_string() }
                    }
                    if let Some(message) = reactive_message {
                        InlineError { message }
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

/// The register page; `bad` carries the code from a javascript-off attempt.
///
/// A successful register logs the new user in and lands home (as the Leptos
/// register page did) — by 303 without a client build, by navigation with one.
#[component]
pub fn Register(
    /// Error code from a failed registration attempt.
    bad: Option<String>,
) -> Element {
    let router = try_use_context::<RouterContext>();
    let session_hint = try_use_context::<Signal<SessionHint>>();
    let mut submit =
        use_action(move |form: ClientRegisterForm| async move { register_client(form).await });

    use_effect(move || {
        if let Some(AuthOutcome::LoggedIn { user }) = answered(submit.value()) {
            record_session_hint(session_hint, SessionHint::SignedIn(Box::new(user)));
            go_home(router);
        }
    });

    let reactive_message = match &answered(submit.value()) {
        Some(AuthOutcome::Failed { code }) => Some(
            register_message(Some(code.as_str()))
                .unwrap_or("Registration failed. Check the details and try again.")
                .to_string(),
        ),
        _ => transport_message(submit.value()),
    };

    rsx! {
        article { class: "my-6 md:container md:mx-auto prose",
            h1 { "Register" }
            form {
                method: "post",
                action: "/auth/register",
                onsubmit: move |evt| {
                    evt.prevent_default();
                    submit.call(register_form_from_values(&evt));
                },
                div { class: "grid gap-6",
                    if let Some(message) = register_message(bad.as_deref()) {
                        InlineError { message: message.to_string() }
                    }
                    if let Some(message) = reactive_message {
                        InlineError { message }
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

/// The logout page. Before a logout it shows the button — posting ends the
/// session and 303s back with `?done`, and the reactive twin ends it and
/// reports `Done` instead.
///
/// After a logout it shows the Leptos page's "Logged out" line instead, and the
/// navbar menu goes back to the guest links through the session hint.
#[component]
pub fn Logout(
    /// Set when the session has just ended.
    done: Option<String>,
) -> Element {
    let session_hint = try_use_context::<Signal<SessionHint>>();
    let mut submit = use_action(move |(): ()| async move { logout_client().await });

    use_effect(move || {
        if matches!(answered(submit.value()), Some(AuthOutcome::Done)) {
            record_session_hint(session_hint, SessionHint::SignedOut);
        }
    });

    let finished = done.is_some() || matches!(answered(submit.value()), Some(AuthOutcome::Done));
    let failure_message = transport_message(submit.value());

    rsx! {
        article { class: "my-6 md:container md:mx-auto prose",
            h1 { "Log out" }
            div { class: "grid gap-6",
                if finished {
                    p { "Logged out" }
                } else {
                    form {
                        method: "post",
                        action: "/auth/logout",
                        onsubmit: move |evt| {
                            evt.prevent_default();
                            submit.call(());
                        },
                        button { class: "btn btn-primary", "Log out" }
                    }
                }
                if let Some(message) = failure_message {
                    InlineError { message }
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
    /// Set when an avatar arrived through the javascript-off upload form.
    uploaded: Option<String>,
    /// Error code from a failed javascript-off upload.
    bad_upload: Option<String>,
) -> Element {
    rsx! {
        document::Title { "Edit Profile" }
        article { class: "my-6 md:container md:mx-auto prose",
            h1 { "Edit Profile" }
            SuspenseBoundary {
                fallback: |_context: SuspenseContext| rsx! { Loading {} },
                ProfileForm {
                    saved,
                    bad,
                    uploaded,
                    bad_upload,
                }
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
    /// Set when an avatar arrived through the javascript-off upload form.
    uploaded: Option<String>,
    /// Error code from a failed javascript-off upload.
    bad_upload: Option<String>,
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
    let avatar_username = user.username.clone();
    rsx! {
        // Leptos placed the avatar picker between the heading and the form.
        // The slug follows the session user, so after a rename the next
        // upload still names the account it belongs to (the service checks
        // it against the session anyway).
        FileUploadWithProgress {
            config: UploadConfiguration::Avatar,
            slug: avatar_username,
            uploaded,
            bad_upload,
        }
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
// Reactive twins of the `/auth/*` handlers (MIGRATION.md Phase 6 slice 6d)
// ---------------------------------------------------------------------------
//
// Same services, same session records and same fixed failure codes as the form
// handlers in `crate::server::forms`; only the delivery differs, because a
// client build has no page load to carry a `?bad=` query and a webview window
// cannot follow a 303 at all.
//
// They live under `/api/client/*` rather than on the Leptos server-fn paths
// (`POST /api/login`, …) because the live Leptos site still owns those until
// Phase 8 retires it, and axum panics on two handlers for one method and path.

/// Logs in and starts the session, answering `LoggedIn` with the user so the
/// client can show them without a second round trip.
///
/// # Errors
/// 500 with a retry message when the session store will not take the session.
#[post("/api/client/login",
    db: crate::server::current_user::Db,
    session: tower_sessions::Session
)]
pub async fn login_client(form: ClientLoginForm) -> core::result::Result<AuthOutcome, HttpError> {
    let user = match services::user::login_user(&db.0, form.username, form.password).await {
        Ok(user) => user,
        Err(error) => {
            tracing::warn!("login twin rejected: {error}");
            return Ok(AuthOutcome::Failed {
                code: login_code(&error).to_string(),
            });
        }
    };

    if let Err(error) = record_login(&session, user.id).await {
        tracing::error!("login twin: could not store the session: {error}");
        return HttpError::internal_server_error("Could not start the session, try again later");
    }
    remember_session(&session, form.remember.as_deref());

    Ok(AuthOutcome::LoggedIn { user })
}

/// Creates the account, then logs the new user in (the register form's order).
///
/// # Errors
/// 500 with a retry message when the session store will not take the session.
#[post("/api/client/register",
    db: crate::server::current_user::Db,
    session: tower_sessions::Session
)]
pub async fn register_client(
    form: ClientRegisterForm,
) -> core::result::Result<AuthOutcome, HttpError> {
    let user = match services::user::register_user_service(
        &db.0,
        RegisterUserForm {
            username: form.username,
            email: form.email,
            password: form.password,
            password_confirmation: form.password_confirmation,
        },
    )
    .await
    {
        Ok(user) => user,
        Err(error) => {
            tracing::warn!("register twin rejected: {error}");
            return Ok(AuthOutcome::Failed {
                code: register_code(&error).to_string(),
            });
        }
    };

    if let Err(error) = record_login(&session, user.id).await {
        tracing::error!("register twin: could not store the session: {error}");
        return HttpError::internal_server_error("Could not start the session, try again later");
    }
    remember_session(&session, form.remember.as_deref());

    Ok(AuthOutcome::LoggedIn { user })
}

/// Ends the signed-in session, answering `Done` (the javascript-off handler
/// gets there with `?done=1` instead).
///
/// # Errors
/// 500 with a retry message when the session store will not flush the session.
#[post("/api/client/logout", session: tower_sessions::Session)]
pub async fn logout_client() -> core::result::Result<AuthOutcome, HttpError> {
    if let Err(error) = record_logout(&session).await {
        tracing::error!("logout twin: could not end the session: {error}");
        return HttpError::internal_server_error("Could not end the session, try again later");
    }
    Ok(AuthOutcome::Done)
}

// ---------------------------------------------------------------------------
// Reactive plumbing shared by the three pages
// ---------------------------------------------------------------------------

/// Navigates home from an auth action's success path. The router handle was
/// looked up during render, so the routerless SSR markup tests stay put.
fn go_home(router: Option<RouterContext>) {
    if let Some(router) = router {
        // The only kind of failure is an external URL, which a `Route` is not.
        let _failure = router.replace(Route::RecordLabelHome {});
    }
}

/// Tells the shell's chrome what an auth action decided about the session (see
/// [`crate::ui::SessionHint`]); absent in the routerless markup tests.
fn record_session_hint(hint: Option<Signal<SessionHint>>, answer: SessionHint) {
    if let Some(mut hint) = hint {
        hint.set(answer);
    }
}

/// The action's outcome, out of the signal dioxus keeps an action's value in.
fn answered(value: Option<Result<ReadSignal<AuthOutcome>, CapturedError>>) -> Option<AuthOutcome> {
    value.and_then(|result| result.ok().map(|value| value.cloned()))
}

/// A transport failure from an auth action (the API was unreachable, or the
/// twin answered 500): the captured error's own text, as the links editor
/// shows it — for these endpoints the message is always ours, never the
/// database's.
fn transport_message(
    value: Option<Result<ReadSignal<AuthOutcome>, CapturedError>>,
) -> Option<String> {
    value.and_then(|result| result.err().map(|error| error.to_string()))
}

/// A form submit's text inputs as a JSON object. Dioxus' `parsed_values`
/// deserializer sits behind a dioxus-html feature the meta crate does not
/// expose, so the fields are mapped by hand, as in `ui::admin::links`.
fn text_fields(evt: &FormEvent) -> serde_json::Map<String, serde_json::Value> {
    let mut fields = serde_json::Map::new();
    for (name, value) in evt.values() {
        if let FormValue::Text(text) = value {
            fields.insert(name, serde_json::Value::String(text));
        }
    }
    fields
}

/// The login page's fields for the reactive path. `remember` only arrives when
/// the box is ticked, exactly as in the form-encoded post; an unparsable body
/// becomes an empty form, which the service rejects as `required`.
fn login_form_from_values(evt: &FormEvent) -> ClientLoginForm {
    serde_json::from_value(serde_json::Value::Object(text_fields(evt))).unwrap_or_default()
}

/// The register page's fields for the reactive path.
fn register_form_from_values(evt: &FormEvent) -> ClientRegisterForm {
    serde_json::from_value(serde_json::Value::Object(text_fields(evt))).unwrap_or_default()
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
