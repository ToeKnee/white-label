//! Remembering the login across desktop restarts.
//!
//! A web browser already remembers: the login form's "remember me" makes
//! the session cookie `Max-Age`d (see `server::current_user::remember_session`),
//! so the browser itself persists the cookie across restarts. The desktop
//! client has no such store — dioxus keeps its cookies in a process-wide
//! `reqwest` jar that dies with the window.
//!
//! This module supplies the framework-free decisions the desktop glue
//! (`desktop`) acts on: given the `Set-Cookie` lines the client has just
//! received, decide whether the session is a *remembered* one (has a
//! positive `Max-Age`) worth saving to the OS credential store, a
//! *cleared* one worth removing, or not this app's session cookie at
//! all; and given a saved entry, re-derive a cookie whose remaining
//! life matches what the browser would have sent at the same moment.
//!
//! entry is only as strong as the credential store, so non-remembered
//! session cookies are deliberately never written: closing the window
//! should end the session, exactly as closing a private browser window does.

/// Desktop glue: capture the `Set-Cookie` lines and sync the store.
#[cfg(feature = "desktop")]
pub mod desktop;

use serde::{Deserialize, Serialize};

/// Credential-store service name, matching the `[bundle]` identifier in
/// `Dioxus.toml` (the app's permanent identity).
pub const KEYRING_SERVICE: &str = "net.ynottony.whitelabel";

/// The session cookie's name, as set by `tower-sessions` in
/// `server::mod` (its default cookie name).
pub const SESSION_COOKIE_NAME: &str = "id";

/// A remembered session, as kept in the OS credential store.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedSession {
    /// The API origin the cookie was issued for. Saved entries are keyed by
    /// origin in the credential store, but the check in
    /// [`restore_cookie`] keeps a hand-edited or moved entry from ever
    /// authenticating against an origin it was not issued for.
    pub origin: String,
    /// The whole `Set-Cookie` line, verbatim as the server sent it, so
    /// `Secure`, `HttpOnly` and `SameSite` survive the round trip.
    pub cookie: String,
    /// When the line was saved (unix seconds), so [`restore_cookie`] can
    /// age the `Max-Age` down to the remaining lifetime.
    pub saved_at_unix: i64,
}

/// Serialise a session for the credential store.
///
/// # Errors
/// Only ever a [`serde_json`] error for types this struct cannot contain.
pub fn encode_saved(saved: &SavedSession) -> serde_json::Result<String> {
    serde_json::to_string(saved)
}

/// Reverse of [`encode_saved`]; malformed or hand-edited entries are
/// rejected rather than trusted.
///
/// # Errors
/// A [`serde_json`] error when the stored text is not a saved session.
pub fn decode_saved(stored: &str) -> serde_json::Result<SavedSession> {
    serde_json::from_str(stored)
}

/// What the credential store should do with a session, given the
/// `Set-Cookie` lines the client has received since the last check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CookieDecision {
    /// A remembered session cookie to save, kept verbatim.
    Save {
        /// The whole `Set-Cookie` line as the server sent it.
        cookie: String,
    },
    /// The server expired or removed the session cookie: drop any saved
    /// entry.
    Clear,
    /// None of the lines mentioned this app's session cookie: leave the
    /// store alone.
    Ignore,
}

/// Decide what to do with the credential store, given every `Set-Cookie`
/// line the client saw since the last decision.
///
/// The last line mentioning the session cookie wins, which covers a login
/// and a logout arriving before either was acted on.
#[must_use]
pub fn decide(lines: &[String]) -> CookieDecision {
    let mut decision = CookieDecision::Ignore;
    for line in lines {
        let Some(cookie) = parse_set_cookie(line) else {
            continue;
        };
        if cookie.name() != SESSION_COOKIE_NAME {
            continue;
        }
        // A cookie with no value is a removal; one without a positive
        // `Max-Age` is a session cookie that dies with the process, so
        // there is nothing worth persisting either way.
        let remembered = !cookie.value().is_empty()
            && max_age_of(&cookie.attributes).is_some_and(|seconds| seconds > 0);
        decision = if remembered {
            CookieDecision::Save {
                cookie: line.clone(),
            }
        } else {
            CookieDecision::Clear
        };
    }
    decision
}

/// Re-derive the saved cookie for use right now.
///
/// Returns `None` when the entry was saved for a different origin, or when
/// the remaining lifetime is up (a login remembered for 30 days stops
/// working after 30 days, just like in a browser). A `saved_at_unix` in
/// the future — a clock that has since been put back — ages nothing.
///
/// The returned line keeps every attribute verbatim except `Max-Age`,
/// which is rewritten to the seconds left; duplicate `Max-Age`s are
/// dropped so the remaining lifetime cannot be shadowed.
#[must_use]
pub fn restore_cookie(saved: &SavedSession, api_origin: &str, now_unix: i64) -> Option<String> {
    if saved.origin != api_origin {
        return None;
    }
    let cookie = parse_set_cookie(&saved.cookie)?;
    let max_age = max_age_of(&cookie.attributes)?;
    // A `saved_at_unix` far off the real timeline (a clock put back,
    // or a hand-edited entry) must never age nothing into an extension:
    // a failed subtraction counts as fully elapsed, a future stamp as no
    // elapsed time.
    let elapsed = now_unix
        .checked_sub(saved.saved_at_unix)
        .unwrap_or(i64::MAX)
        .max(0);
    let remaining = max_age.saturating_sub(elapsed);
    if remaining <= 0 {
        return None;
    }

    let mut rebuilt = String::from(cookie.name_value);
    let mut rewrote_max_age = false;
    for attribute in &cookie.attributes {
        if attribute_name(attribute).eq_ignore_ascii_case("max-age") {
            if !rewrote_max_age {
                rebuilt.push_str("; Max-Age=");
                rebuilt.push_str(&remaining.to_string());
                rewrote_max_age = true;
            }
        } else {
            rebuilt.push_str("; ");
            rebuilt.push_str(attribute);
        }
    }
    Some(rebuilt)
}

/// A parsed `Set-Cookie` line, borrowed from the line it came from.
struct ParsedSetCookie<'a> {
    /// The `name=value` part, verbatim.
    name_value: &'a str,
    /// Each attribute, verbatim and trimmed.
    attributes: Vec<&'a str>,
}

impl<'a> ParsedSetCookie<'a> {
    fn name(&self) -> &'a str {
        match self.name_value.split_once('=') {
            Some((name, _)) => name,
            None => self.name_value,
        }
    }

    fn value(&self) -> &'a str {
        match self.name_value.split_once('=') {
            Some((_, value)) => value,
            None => "",
        }
    }
}

/// Split a `Set-Cookie` line into its `name=value` part and attributes.
/// A line with no `=` in its first segment is not a cookie at all.
fn parse_set_cookie(line: &str) -> Option<ParsedSetCookie<'_>> {
    let mut segments = line.split(';');
    let name_value = segments.next()?.trim();
    if !name_value.contains('=') {
        return None;
    }
    Some(ParsedSetCookie {
        name_value,
        attributes: segments.map(str::trim).collect(),
    })
}

/// The value of the first parseable `Max-Age` attribute, if any.
fn max_age_of(attributes: &[&str]) -> Option<i64> {
    attributes.iter().find_map(|attribute| {
        let (_, value) = attribute.split_once('=')?;
        if attribute_name(attribute).eq_ignore_ascii_case("max-age") {
            value.trim().parse().ok()
        } else {
            None
        }
    })
}

/// The part of an attribute before its `=` (the whole attribute when it
/// has none, as for `HttpOnly`).
fn attribute_name(attribute: &str) -> &str {
    match attribute.split_once('=') {
        Some((name, _)) => name.trim(),
        None => attribute.trim(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COOKIE_30_DAYS: &str =
        "id=tok32; Max-Age=2592000; Path=/; HttpOnly; Secure; SameSite=Strict";

    fn line(value: &str, max_age: Option<&str>) -> String {
        max_age.map_or_else(
            || format!("{SESSION_COOKIE_NAME}={value}"),
            |age| format!("{SESSION_COOKIE_NAME}={value}; Max-Age={age}"),
        )
    }

    #[test]
    fn saved_session_round_trips() {
        let saved = SavedSession {
            origin: "http://127.0.0.1:8127".to_owned(),
            cookie: COOKIE_30_DAYS.to_owned(),
            saved_at_unix: 1_700_000_000,
        };
        let encoded = encode_saved(&saved).expect("serialisable struct");
        let decoded = decode_saved(&encoded).expect("round trip");
        assert_eq!(decoded, saved);
    }

    #[test]
    fn garbage_stored_text_is_rejected() {
        assert!(decode_saved("not json").is_err());
        // Valid JSON, wrong shape: still rejected.
        assert!(decode_saved(r#"{"origin":"http://x"}"#).is_err());
    }

    #[test]
    fn remember_me_line_is_saved_verbatim() {
        assert_eq!(
            decide(&[line("tok", Some("2592000"))]),
            CookieDecision::Save {
                cookie: line("tok", Some("2592000"))
            }
        );
    }

    #[test]
    fn max_age_attributes_are_case_insensitive() {
        let mixed_case = format!("{SESSION_COOKIE_NAME}=tok; max-age=100; path=/");
        assert_eq!(
            decide(std::slice::from_ref(&mixed_case)),
            CookieDecision::Save { cookie: mixed_case }
        );
    }

    #[test]
    fn session_cookie_without_max_age_is_cleared_not_saved() {
        // A session cookie (no Max-Age) dies with the process: nothing to
        // persist, and it must also clear anything remembered earlier.
        assert_eq!(decide(&[line("tok", None)]), CookieDecision::Clear);
    }

    #[test]
    fn expired_and_removed_cookies_clear_the_entry() {
        assert_eq!(decide(&[line("tok", Some("0"))]), CookieDecision::Clear);
        assert_eq!(decide(&[line("tok", Some("-1"))]), CookieDecision::Clear);
        assert_eq!(decide(&[line("", Some("0"))]), CookieDecision::Clear);
        assert_eq!(decide(&[line("", None)]), CookieDecision::Clear);
    }

    #[test]
    fn lines_about_other_cookies_are_ignored() {
        assert_eq!(decide(&[]), CookieDecision::Ignore);
        // A prefix of the session name is not a match, and neither are
        // unrelated cookies.
        assert_eq!(
            decide(&["idnope=tok; Max-Age=2592000".to_owned()]),
            CookieDecision::Ignore
        );
        assert_eq!(
            decide(&["theme=dark; Max-Age=999999".to_owned()]),
            CookieDecision::Ignore
        );
        assert_eq!(decide(&["not a cookie".to_owned()]), CookieDecision::Ignore);
    }

    #[test]
    fn the_last_session_line_wins_either_way() {
        let remembered = line("tok", Some("2592000"));
        let removed = line("", Some("0"));
        assert_eq!(
            decide(&[removed.clone(), remembered.clone()]),
            CookieDecision::Save {
                cookie: remembered.clone()
            }
        );
        assert_eq!(decide(&[remembered, removed]), CookieDecision::Clear);
    }

    #[test]
    fn restore_ages_the_max_age_down() {
        let day = 86_400;
        let saved = SavedSession {
            origin: "http://127.0.0.1:8127".to_owned(),
            cookie: COOKIE_30_DAYS.to_owned(),
            saved_at_unix: 1_000,
        };
        let restored = restore_cookie(&saved, "http://127.0.0.1:8127", 1_000 + day)
            .expect("a day into 30 days is still alive");
        assert_eq!(
            restored,
            "id=tok32; Max-Age=2505600; Path=/; HttpOnly; Secure; SameSite=Strict"
        );
    }

    #[test]
    fn restore_rejects_the_wrong_origin() {
        let saved = SavedSession {
            origin: "http://127.0.0.1:8127".to_owned(),
            cookie: COOKIE_30_DAYS.to_owned(),
            saved_at_unix: 0,
        };
        assert_eq!(restore_cookie(&saved, "https://label.example", 10), None);
    }

    #[test]
    fn restore_rejects_an_expired_entry() {
        let saved = SavedSession {
            origin: "http://127.0.0.1:8127".to_owned(),
            cookie: line("tok", Some("100")),
            saved_at_unix: 1_000,
        };
        assert_eq!(restore_cookie(&saved, "http://127.0.0.1:8127", 1_100), None);
        assert!(restore_cookie(&saved, "http://127.0.0.1:8127", 1_099).is_some());
    }

    #[test]
    fn restore_survives_clock_being_set_backwards() {
        let saved = SavedSession {
            origin: "http://127.0.0.1:8127".to_owned(),
            cookie: line("tok", Some("100")),
            saved_at_unix: 2_000,
        };
        let restored = restore_cookie(&saved, "http://127.0.0.1:8127", 1_000)
            .expect("a future saved_at ages nothing");
        assert_eq!(restored, line("tok", Some("100")));
    }

    #[test]
    fn an_absurd_saved_at_stamp_counts_as_expired() {
        // A hand-edited entry must not overflow the subtraction into a
        // cookie that lives for eons.
        let saved = SavedSession {
            origin: "http://127.0.0.1:8127".to_owned(),
            cookie: line("tok", Some("100")),
            saved_at_unix: i64::MIN,
        };
        assert_eq!(restore_cookie(&saved, "http://127.0.0.1:8127", 0), None);
    }

    #[test]
    fn restore_drops_duplicate_max_age_entries() {
        // A second `Max-Age` must not survive to shadow the rewritten one.
        let saved = SavedSession {
            origin: "http://127.0.0.1:8127".to_owned(),
            cookie: format!("{SESSION_COOKIE_NAME}=tok; Max-Age=99999999; max-age=2592000; Path=/"),
            saved_at_unix: 0,
        };
        let restored = restore_cookie(&saved, "http://127.0.0.1:8127", 0)
            .expect("99999999 is the first parseable Max-Age");
        assert_eq!(
            restored,
            format!("{SESSION_COOKIE_NAME}=tok; Max-Age=99999999; Path=/")
        );
    }
}
