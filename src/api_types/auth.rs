//! Response types for the reactive auth endpoints (`/api/client/*`).

use crate::models::auth::User;

/// What an auth action answered.
///
/// Outcomes that a form could also fail with travel as data rather than HTTP
/// statuses: the javascript-off handlers report them as `?bad=<code>` inside a
/// 303, and a webview window cannot follow a 303 at all, so the reactive twins
/// carry the very same fixed codes and reuse the alert tables in
/// `crate::ui::auth`. A transport failure (session store down, unreachable
/// API) is still an error at the request level.
///
/// ```text
/// POST /api/client/login  {"form":{"username":"test-1","password":"password"}}
/// 200 {"LoggedIn":{"user":{…}}}              session cookie set
///
/// POST /api/client/login  {"form":{"username":"test-1","password":"nope"}}
/// 200 {"Failed":{"code":"password"}}         no session cookie
/// ```
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, Eq, PartialEq)]
pub enum AuthOutcome {
    /// A session started for this user (login and register).
    LoggedIn {
        /// The signed-in user, so the client can show them without refetching.
        user: User,
    },
    /// The action succeeded and there is no session user to hand back.
    Done,
    /// The action was refused. `code` is one of the fixed codes `ui::auth`
    /// already maps onto alert text (`not_found`, `password`, `taken`, …).
    Failed {
        /// The fixed failure code, never a message from the database.
        code: String,
    },
}
