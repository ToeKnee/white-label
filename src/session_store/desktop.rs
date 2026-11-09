//! Desktop-only glue: keeps the credential store in step with the cookie
//! jar, so a "remember me" login survives restarts.
//!
//! Dioxus builds its `reqwest` client lazily with a process-wide cookie
//! jar that is lost at exit. [`prepare`] installs our client instead (via
//! dioxus-fullstack's `GLOBAL_REQUEST_CLIENT` hook) before the first
//! server fn call: a jar that wraps the usual one and *captures* every
//! `Set-Cookie` header, plus — if one is remembered — a re-seeded cookie.
//!
//! Saving and clearing is driven by the session hint, not by sniffing
//! traffic: the [`SessionPersistence`] component reacts to the same
//! [`SessionHint`] the navbar menu rides, and only then consults what the
//! jar captured. See [`crate::session_store`] for the decision rules.
//!
//! The entry lives in the platform's credential store: Keychain Services
//! on macOS, Credential Manager on Windows, the Secret Service (GNOME
//! Keyring, KWallet) on Linux — via the `keyring` crate's `v1` mode,
//! which is the app-facing wrapper over `keyring-core`. A machine without
//! any of them (a bare Linux session with no Secret Service) gets a
//! warning and an ordinary non-remembered login.

// Dioxus component functions are conventionally PascalCase, and `Element`
// is a type alias for a `Result`, which trips pedantic lints that do not
// apply to components (this one never returns an error variant).
#![expect(
    non_snake_case,
    reason = "dioxus components are PascalCase by convention and are called as components from `ui::shell`"
)]
#![expect(
    clippy::missing_errors_doc,
    reason = "`Element` is a `Result` alias by type, not by API: components never return the error variant"
)]

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::PoisonError;

use dioxus::fullstack::reqwest;
use dioxus::prelude::*;
use reqwest::Url;
use reqwest::cookie::CookieStore;
use reqwest::cookie::Jar;
use reqwest::header::HeaderValue;

use crate::session_store::CookieDecision;
use crate::session_store::KEYRING_SERVICE;
use crate::session_store::SavedSession;
use crate::session_store::decide;
use crate::session_store::decode_saved;
use crate::session_store::encode_saved;
use crate::session_store::restore_cookie;
use crate::ui::SessionHint;
use crate::ui::use_session_hint;

/// The capturing jar in use, once [`prepare`] has installed it.
/// Syncs read `Set-Cookie` lines out of it; it stays unset (and syncs
/// are no-ops) when there is no credential store or no client was installed.
static JAR: OnceLock<Arc<CapturingJar>> = OnceLock::new();

/// The API origin [`prepare`] was run for, also the credential entry's
/// user name: one entry per origin, so pointing the client at a different
/// server never shows (or clobbers) another server's session.
static ORIGIN: OnceLock<String> = OnceLock::new();

/// Now, in unix seconds, for the aging in [`crate::session_store`].
fn now_unix() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Wraps the jar dioxus would have used, capturing the full `Set-Cookie`
/// lines on the way through.
///
/// The jar's own read-back gives only `name=value`, and it is the
/// attributes — `Max-Age` in particular — that tell a remembered session
/// cookie from one meant to die with the process.
struct CapturingJar {
    jar: Jar,
    /// `Set-Cookie` lines received since the last sync, in order.
    captured: Mutex<Vec<String>>,
}

impl CapturingJar {
    /// Take everything captured so far, leaving the jar's capture empty.
    fn take_captured(&self) -> Vec<String> {
        let mut captured = self.captured.lock().unwrap_or_else(PoisonError::into_inner);
        std::mem::take(&mut captured)
    }

    /// Put a restored cookie into the wrapped jar, as if the server had
    /// just set it.
    fn seed(&self, cookie: &str, url: &Url) {
        self.jar.add_cookie_str(cookie, url);
    }
}

impl CookieStore for CapturingJar {
    fn set_cookies(&self, cookie_headers: &mut dyn Iterator<Item = &HeaderValue>, url: &Url) {
        // Collect once: capture from the owned copies, then replay the
        // same headers into the wrapped jar.
        let headers: Vec<HeaderValue> = cookie_headers.cloned().collect();
        let lines: Vec<String> = headers
            .iter()
            .filter_map(|header| header.to_str().ok().map(str::to_owned))
            .collect();
        if !lines.is_empty() {
            self.captured
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .extend(lines);
        }
        self.jar.set_cookies(&mut headers.iter(), url);
    }

    fn cookies(&self, url: &Url) -> Option<HeaderValue> {
        self.jar.cookies(url)
    }
}

/// Seed the cookie jar from the credential store and install the client
/// that keeps it, before dioxus lazily builds its own default one.
///
/// Must run after `set_server_url` (the store is keyed by the API origin)
/// and before the first server fn call (which initialises dioxus'
/// `GLOBAL_REQUEST_CLIENT`). Failure at any step is logged and leaves the
/// app with a normal, non-remembered login.
pub fn prepare(api_origin: &'static str) {
    let Ok(url) = Url::parse(api_origin) else {
        tracing::warn!("api origin {api_origin} is not a valid url: login will not be remembered");
        return;
    };
    if let Err(problem) = keyring::Entry::store_status() {
        tracing::warn!("no credential store available ({problem}): login will not be remembered");
        return;
    }

    let jar = Arc::new(CapturingJar {
        jar: Jar::default(),
        captured: Mutex::new(Vec::new()),
    });
    restore_into(&jar, &url, api_origin);

    let client = reqwest::Client::builder()
        // The provider alone activates cookie handling for this jar;
        // calling `cookie_store(true)` as well would replace it with
        // reqwest's default jar.
        .cookie_provider(Arc::clone(&jar))
        .build();
    match client {
        Ok(client) => {
            if dioxus::fullstack::GLOBAL_REQUEST_CLIENT
                .set(client)
                .is_err()
            {
                tracing::warn!(
                    "a request client was already installed: login will not be remembered"
                );
                return;
            }
        }
        Err(problem) => {
            tracing::warn!(
                "could not build the request client ({problem}): login will not be remembered"
            );
            return;
        }
    }
    let _ = JAR.set(jar);
    let _ = ORIGIN.set(api_origin.to_owned());
}

/// Load this origin's credential entry and, if it is still alive, seed it
/// into the jar. Dead or unreadable entries are deleted, so the store does
/// not accumulate entries the user can never use.
fn restore_into(jar: &CapturingJar, url: &Url, api_origin: &str) {
    let entry = match keyring::Entry::new(KEYRING_SERVICE, api_origin) {
        Ok(entry) => entry,
        Err(problem) => {
            tracing::warn!(
                "could not open the credential entry ({problem}): login will not be remembered"
            );
            return;
        }
    };
    let stored = match entry.get_password() {
        Ok(stored) => stored,
        Err(keyring::Error::NoEntry) => return,
        Err(problem) => {
            tracing::warn!("could not read the remembered session ({problem})");
            return;
        }
    };
    let revived = decode_saved(&stored)
        .map_err(|problem| tracing::warn!("remembered session was unreadable ({problem})"))
        .ok()
        .and_then(|saved| restore_cookie(&saved, api_origin, now_unix()));
    if let Some(cookie) = revived {
        jar.seed(&cookie, url);
        tracing::info!("restored the remembered session for {api_origin}");
    } else {
        tracing::info!("the remembered session for {api_origin} has expired");
        discard_entry(&entry);
    }
}

/// Bring the credential entry in line with what the server just did with
/// the session cookie, as decided from the jar's captured lines.
fn sync_from_responses() {
    let (Some(jar), Some(origin)) = (JAR.get(), ORIGIN.get()) else {
        return;
    };
    match decide(&jar.take_captured()) {
        CookieDecision::Ignore => {}
        CookieDecision::Save { cookie } => {
            let saved = SavedSession {
                origin: origin.clone(),
                cookie,
                saved_at_unix: now_unix(),
            };
            match encode_saved(&saved) {
                Ok(stored) => match keyring::Entry::new(KEYRING_SERVICE, origin) {
                    Ok(entry) => match entry.set_password(&stored) {
                        Ok(()) => tracing::info!("login for {origin} is now remembered"),
                        Err(problem) => tracing::warn!("could not remember the login ({problem})"),
                    },
                    Err(problem) => {
                        tracing::warn!("could not open the credential entry ({problem})");
                    }
                },
                // Cannot happen for this flat struct; if it somehow did,
                // there is simply nothing to store.
                Err(problem) => tracing::warn!("could not serialise the session ({problem})"),
            }
        }
        CookieDecision::Clear => match keyring::Entry::new(KEYRING_SERVICE, origin) {
            Ok(entry) => discard_entry(&entry),
            Err(problem) => {
                tracing::warn!("could not open the credential entry ({problem})");
            }
        },
    }
}

/// Delete an entry, ignoring the "there was nothing to delete" case that
/// a double logout can produce.
fn discard_entry(entry: &keyring::Entry) {
    match entry.delete_credential() {
        Ok(()) => tracing::info!("the remembered login was cleared"),
        Err(keyring::Error::NoEntry) => {}
        Err(problem) => tracing::warn!("could not clear the remembered login ({problem})"),
    }
}

/// Invisible component that keeps the credential store in step with the
/// session: it rides the same [`SessionHint`] the navbar menu does, and
/// syncs after a reactive sign-in or sign-out.
///
/// Mount once from `App`, inside its scope. `SessionHint::Unknown` is
/// deliberately a no-op: at startup the hint is `Unknown` while the jar
/// may have just been seeded from the store, and syncing then would find
/// no `Set-Cookie` headers and (correctly!) decide to clear the entry it
/// only just restored.
pub fn SessionPersistence() -> Element {
    let session_hint = use_session_hint();
    use_effect(move || match session_hint() {
        // Nothing has been signed in or out since launch.
        SessionHint::Unknown => {}
        // A login, a register, or the profile page passing a session
        // change on: the response's `Set-Cookie` handling has already run
        // by now, so the jar holds the lines to decide from. A change
        // that set no cookie (an avatar upload, say) decides `Ignore`,
        // and the store is left alone.
        SessionHint::SignedIn(_) | SessionHint::SignedOut => sync_from_responses(),
    });
    rsx! {}
}
