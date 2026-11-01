//! Types shared between the client and the server.
//!
//! Server function arguments and return values must be serializable, so these
//! live outside the server-only `auth` module (unlike the Leptos app, which
//! shares a single `models` module).

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// A logged-in user. Same shape as the Leptos app's `models::auth::User`,
/// minus the timestamps (not needed for the spike's auth checks).
///
/// Note: the Leptos app's SSR `Authentication` impl has `is_authenticated`
/// inverted (`id == -1` means authenticated). The spike follows the standard
/// semantics (see `auth.rs`); the inversion on main is flagged in the README
/// as an open question for the port.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: String,
    pub description: Option<String>,
    pub avatar: Option<String>,
    pub permissions: HashSet<String>,
}

/// Artist summary for the nested admin routes (real row from `artists`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Artist {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
}
