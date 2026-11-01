# Phase 0 spike — Dioxus 0.7 fullstack against the live backend

Validation-only crate for the Leptos → Dioxus migration (see `../MIGRATION.md`).
It ports the main app's risky mechanics — session auth, server functions,
resources/SSR, nested routing, streaming uploads + progress — against the same
Postgres DB (`users`, `user_permissions`, `axum_sessions`) the Leptos app uses.

## How to run

```sh
cd spike
npm install && npm run css     # builds assets/main.css (Tailwind 4 + daisyUI 5)
cargo run                      # serves 127.0.0.1:8080 by default
```

`dioxus::serve` reads the `IP` and `PORT` env vars, and the public-asset dir
from `DIOXUS_PUBLIC_PATH` (a plain `cargo run` panics without one; `dx serve`
sets both automatically). `run-gate.sh` automates the whole runtime checklist
on port 8123 and captures SSR/auth/router/wire-format output to /tmp/spike-run.

Compile gates (all pass, zero warnings):

```sh
cargo test                                          # 5 unit tests
cargo clippy --all-targets -- -D warnings
cargo check --no-default-features --features web    # client-only host build
cargo check --target wasm32-unknown-unknown --no-default-features --features web
cargo check --no-default-features --features desktop
```

Desktop was compile-validated only (headless environment); the desktop client
would call the server URL set by `set_server_url` in `src/main.rs`.

## Gate results (validated against dioxus 0.7.10, dx CLI 0.7.6)

| Gate item                                | Outcome                                                                                                                                                                                                                                                                                                                             |
| ---------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| axum_session + axum_session_auth pattern | Works; layer order Session → Auth → `Extension(Db)`, see `src/main.rs`                                                                                                                                                                                                                                                              |
| Router feature combo                     | Umbrella `dioxus = { features = ["fullstack", "router"] }` is enough — no separate `dioxus-router` dep, and **not** the router's `streaming` feature (tested both ways)                                                                                                                                                             |
| Server-side state (pool)                 | `Extension<Db>` newtype passed via macro attr; access with `db.0.0` — chosen over `Lazy`/`State` to dodge the orphan rule on remote types                                                                                                                                                                                           |
| Upload extra fields                      | Query-string extractors: `#[post("/api/upload_file?file_type&slug", …)]` + `FileStream` body — port of the multipart "type then slug then file" convention                                                                                                                                                                          |
| Progress streaming                       | **Verified end-to-end**: 8 MiB authenticated upload returned `8388608`; concurrent `GET /api/file_progress?filename=blob.bin` streamed growing byte counts. The tracker keys the _original_ filename; the stored file is sanitized (`uploads/artists/spiketest-blob_bin`)                                                           |
| Trailing slashes                         | Parsing is tolerant (both present/absent parse; unit-tested); `ArtistOverview` _renders_ canonically with a trailing slash because its nest route is `/`                                                                                                                                                                            |
| Session wire behaviour                   | Logout rotates the `session` cookie and clears `store`; sessions are interchangeable with the running Leptos app (same `axum_sessions` table). A one-off 300 s hang at logout during the auth gate did **not** reproduce in isolation (unauth logout 200, authed logout 200, later `GET /api/user` → `null`) — treated as transient |
| SSR + `use_resource`                     | **Needs `SuspenseBoundary` + `Resource::suspend()?`** — see findings below                                                                                                                                                                                                                                                          |

## Big findings

### 1. Layouts use `Outlet`, not a `children` prop (silent breakage)

`#[layout(AppShell)]` generates `AppShell { …nest params… }` with **no
children**. A `children: Element` prop silently defaults to empty and pages
never render (SSR shows an empty `<main>`; even a plain `VirtualDom` unit test
reproduces it). Layouts must render `Outlet::<Route> {}` instead. This is the
main structural difference from Leptos `parent_route!`.

### 2. SSR only awaits resources that `suspend()`

A plain `resource()` read during SSR returns `None` (the future is never
polled server-side) and the client fills content in after hydration — that
would diverge from the Leptos app's SSR. The idiomatic pattern:

```rust
// outer component
SuspenseBoundary {
    fallback: |_cx: SuspenseContext| rsx! { p { class: "animate-pulse", "Loading..." } },
    ArtistsList {}
}

// inner component (must be a separate scope)
let resource = use_resource(api::list_artists);
let artists = resource.suspend()?;   // registers the future with the renderer
match artists() { /* Ok(list) / Err(error) */ }
```

With this, the server fn runs **in-process during SSR** (no HTTP loopback,
extractors get the real request's session/db) and the resolved data is inlined
into the initial chunk — verified with real DB-backed artist lists and the
navbar user menu.

### 3. Server-fn error type decides the HTTP status

- Prelude `Result<T>` (= `CapturedError`): every error is HTTP **500**; the
  intended code appears only in the JSON body (`{"message":…,"code":404}`).
- Explicit `core::result::Result<T, HttpError>`: real HTTP status (401/404/…),
  same JSON body. `HttpError::bad_request("…")` constructors return a ready
  `Result<T, HttpError>` for any `T`, so bail-outs are
  `return HttpError::not_found(format!("…"));` — no helper needed.

All spike server fns use the `HttpError` variant; recommend it app-wide for
API parity with the Leptos server.

### 4. Success/error wire formats

- Success: plain JSON of the Ok value (`null` for `Ok(None)`, e.g.
  `GET /api/user` unauthenticated → `null` + 200).
- Error: `{"message": "…", "code": 401}` with the real status (when the
  server fn uses `HttpError`).
- Login with bogus creds: 401 in both JSON-arg variants (omitted vs
  `"remember": null`), proving multi-arg JSON body handling.

### 5. `login_user` does not populate `current_user` in the same request

In `axum_session_auth` 0.20.1, `auth.login_user(id)` + `remember_user(...)` persist
the session (cookie valid from the **next** request), but `auth.current_user` stays
`None` for the request that logged in. Reading it right after login yields a false
error. Idiomatic fix (see `src/api.rs` `login`): return the user loaded explicitly:

```rust
<User as axum_session_auth::Authentication<User, i64, sqlx::PgPool>>::load_user(user_id, Some(&db.0.0)).await?
```

### 6. Router params and nesting

`#[nest("/admin/artist/:artist_slug")] #[layout(ArtistAdmin)] …` works; the
nest param reaches layout and leaf components, and into server fns called
under SSR. Client-side `Route::from_str`/`to_string` round-trips are unit
tested in `src/routes.rs` (top-level, nested with both params, trailing-slash
tolerance, canonical URL forms).

### 7. Dependency risk: axum_session version yank storm

The maintainer yanked nearly the entire version history of `axum_session`
(only 0.20.0/0.20.1/0.21.0/0.22.1 remain installable), `axum_session_auth`
(no 0.20.1), and `axum_session_sqlx` (no 0.9.1). The main app still builds
only because `Cargo.lock` pins 0.16.0 — a fresh CI build without the lockfile
would break. The spike pins the co-resident set: `axum_session = 0.20.1`,
`axum_session_auth = 0.20.0`, `axum_session_sqlx = 0.9.0` (all axum 0.8 /
sqlx 0.8 compatible). Newer `axum_session` (0.21+) or the sqlx store (0.10+)
require sqlx 0.9, which the app is not on.

### 8. Smaller notes

- `current_user` is `Option<User>` (not `Arc`); `login_user`/`remember_user`/
  `logout_user` as expected; the spike's `Authentication` uses standard
  `is_authenticated()` semantics (`id != -1`). Note: the **main app's SSR
  `Authentication` impl returns `id == -1`** — inverted — a latent quirk to
  keep in mind when porting.
- `Action::call()` returns `Dispatching<()>`; `Action::value()` yields
  `Some(Ok(sig))` where `sig()` is the result — Login page pattern in
  `src/routes.rs`.
- Forms: `event.get_first(name)` → `Option<FormValue>`; `FormValue` is an enum
  (`FormValue::Text(String)`); `event.files()` returns `Vec<FileData>`
  directly. Checkbox checked state posts as text.
- rsx specifics: hyphenated attrs must be quoted (`"data-theme": "light"`),
  text interpolation cannot contain nested quotes, handlers touching signals
  need `move`.
- `asset!("/assets/main.css")` in a debug server build emits an absolute
  filesystem href — fine for `dx serve`/release (embedded), but a bare
  `cargo run` won't serve the CSS (hence the empty `DIOXUS_PUBLIC_PATH` trick).
- dx CLI installed is 0.7.6 while crates are 0.7.10 — gates were run with
  cargo, not dx; upgrade dx before Phase 1 UI work.

Authenticated upload/progress was gated with a throwaway `label_owner` user
seeded directly into the dev DB (bcrypt hash insert; user deleted afterwards —
never commit test credentials). The upload is a **raw-body** POST, not
multipart, over HTTP:

```sh
curl -b jar -X POST 'HOST/api/upload_file?file_type=artist_image&slug=spiketest' \
  -H 'Content-Disposition: attachment; filename="blob.bin"' \
  -H 'content-type: application/octet-stream' \
  -H 'X-Content-Size: 8388608' --data-binary @blob.bin
curl -b -N 'HOST/api/file_progress?filename=blob.bin'   # items are concatenated, no newlines
```

## Open items

- Desktop runtime smoke (needs a display); iOS/Android not attempted yet.
- A pre-commit hook update (`leptosfmt` → dioxus fmt) is a Phase 8 concern.

## Files

- `src/main.rs` — dual-mode entry (server: hand-built axum router + layers;
  client: `launch` + `set_server_url` for desktop).
- `src/auth.rs` — `Authentication` impl over the app's real `users`/
  `user_permissions` tables; `Session` type alias.
- `src/db.rs` — `Db(PgPool)` newtype for the extractor (orphan-rule-safe).
- `src/api.rs` — server fns mirroring the app's `/api` endpoints.
- `src/routes.rs` — router, layouts (`Outlet`), `SuspenseBoundary`/`suspend()`
  SSR pattern, login form via `use_action`, upload demo, unit tests.
- `run-gate.sh` — automated runtime gate (port 8123).
