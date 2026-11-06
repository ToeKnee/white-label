# White Label - a record label management website

> **Migration note:** The project is currently a Leptos 0.8 SSR app. A plan to migrate to
> Dioxus (web + desktop + iOS/Android from one codebase) is in [MIGRATION.md](MIGRATION.md).
> Until the migration lands, all notes below describe the Leptos setup.

## Migration checkpoint (branch `dioxus`)

The Dioxus port (Phases 1–5 of MIGRATION.md, and now the desktop target of
Phase 6) lives in
`src/server/` (axum router: sessions, uploads, current-user extractor, and
the `/api/*` endpoints — including raw-body `upload_file` and streaming
`file_progress`, both in the Leptos server-fn wire format, so the shipped Leptos
UI can call them during the overlap; both stacks finish their uploads through
the shared `services::files::upload` tail) and `src/ui/` (the whole page tree: route
table, layouts, header/footer chrome, public pages, auth pages, the admin
dashboard and the artist/release/track/links/images editors). It boots as a second
binary while the shipped site stays on Leptos (`cargo leptos watch`):

```bash
npm install && npm run css   # tailwind + daisyUI -> public/main.css
cargo run --no-default-features --features server --bin dioxus-server
```

`IP`/`PORT` override the bind address; `DIOXUS_PUBLIC_PATH=public` makes the
server serve `public/main.css`, the favicon and the images (dioxus mounts
each top-level public-dir file at its own path). The `dx` CLI is pinned to
the dioxus crate version (currently 0.7.10).

That SSR-only shape serves **no wasm bundle**: the pages render, navigate with
full reloads, log in and save through the plain-HTML forms, but the reactive
features — including every upload (see the uploads note below) — need the
client build. `sh end2end/run-dioxus-web.sh` builds it and serves the hydrated
site, which is the shape the deployed site will have.

### The wasm client (slice 5e)

`src/ui` also builds for the browser; that bundle is what hydrates the
SSR-rendered pages (SPA navigation, reactive forms, client-side redirects):

```bash
dx build --platform web --bin dioxus-web --features web --no-default-features
```

It writes a **self-contained** directory — every `public/` static plus
`wasm/dioxus-web_bg.wasm` — to `target/dx/dioxus-web/debug/web/public`.
Point the server at it to serve the hydrated site:

```bash
DIOXUS_PUBLIC_PATH=target/dx/dioxus-web/debug/web/public \
  cargo run --no-default-features --features server --bin dioxus-server
```

The gate starts a second `dioxus-server` whose `DIOXUS_PUBLIC_PATH` is that
`dx` output — the bundle's `index.html` is what makes the SSR renderer
inject the wasm bootstrap into every page — so the gate runs two servers:
8127 (SSR-only, `public/`) and 8131 (hydrated). `sh end2end/run-dioxus-web.sh`
does the build-and-serve pair for manual work. Debug bundles are still big
(~27 MB of wasm) but no longer carry the Leptos UI: `src/app`,
`src/components` and `src/routes` are gated to
`any(feature = "ssr", feature = "hydrate")`, so only the Leptos site builds
them. The rest of the weight is the non-optional `leptos*` dependencies
themselves — `src/store` uses `reactive_stores` and `src/utils` still uses
`leptos_router`/`ServerFnError` — which Phase 8 optionalizes or drops.

There is **no `dx serve` web watch mode** for this crate (verified, 0.7.10):
`dx serve --fullstack` builds a _native_ "server build" of the selected bin, so
`dx serve --platform web --bin dioxus-web --features web --no-default-features`
compiles the wasm entry with the web-sys renderer for the host target. The
server then starts and dies at once — `cannot access imported statics on
non-wasm targets` (js-sys) — and the dev server only answers
`500 Backend not ready`. A real watch mode needs either the web target's server
side launched by dioxus itself (`dioxus::launch` under the `server` feature,
which would also have to be handed our axum router for sessions, uploads and
statics) or per-build features (`dx serve`'s `@client`/`@server` argument
lists). Both are open questions in MIGRATION.md; until one lands, the
build-and-serve commands above are the dev flow.

`cargo test --features server` covers the SSR side; the hydrated client side
is covered by `end2end/tests/dioxus_client.spec.ts` (SPA nav issues no extra
document loads, the links editor submits without a round trip, login and logout
submit through the `/api/client/*` twins so the navbar follows the session
without a reload, the profile avatar upload and the artist image upload both
run through the server fns with a progress bar) — it needs the
wasm bundle, so it only runs through the gate script below.

The gate script boots that server against the dev database and adds curl
spot checks, an
idempotent `e2e-dioxus` seed user (registered through the app's own handler),
the admin flows through a `e2e-admin` user you grant `admin`/`label_owner`
to in the dev database once (see the comments in the script; the browser
checks skip without it), an `e2e-tabs` artist row the script seeds
straight into the dev DB with `psql` and **repairs** on every run (its slug
does not match `slugify(name)` on purpose: if a code path ever re-derives a
slug from a name on save, the page pins break instead of drifting quietly), a
chromium navigation + auth smoke test against the SSR-only server, and the
hydrated-client run against the `dx` bundle server (dev DB; ports 8127 and
8131). Page pins go through `pin`/`no_pin`, which print what they checked and
fold failures into the script's exit status — a bare `grep -c` prints `0`
without failing anything:

```bash
sh end2end/run-dioxus-gate.sh
```

(`end2end`'s Playwright is 1.44.1; run `npx playwright install chromium firefox`
in `end2end/` once if the `chromium-1117` / `firefox-1449` builds are missing —
the client spec runs in both browsers now, and an `npx` from elsewhere installs
builds this version cannot launch.)

### Uploads with progress (slices 5f–5g)

`src/ui/upload.rs` holds the picker widget and its two dioxus endpoints. It is
rendered on the profile page (`/profile`, the signed-in user's avatar), the
admin artist **Images** tab (`/admin/artist/<slug>/images`) and the release
editor (`EditRelease`):

- `POST /api/upload?file_type=Avatar&slug=…&filename=…` — the request body is
  the file itself (dioxus sends a `FileStream` argument as the raw body, with
  the mime in `Content-Type`, the size in `X-Content-Size` and the name in
  `Content-Disposition`), which is why the scalars are query params: a server
  fn can take either JSON args or one body arg, never both. Success answers
  the stored file name.
- `GET /api/file_progress?filename` — the running byte totals of your own
  in-flight upload, one per line, until it finishes. Needs only a session.

What a file _is_ is settled by its first bytes, not by what the client said. A
browser derives a file's mime from its name, and the window's `Content-Type` is
invented by dioxus from a table of _web asset_ types that holds no images at all
(a `.png` arrives as `text/html; charset=utf-8`), so a header that is not one of
the accepted types falls back to the file's extension. Both upload paths then
read back the first bytes of the temp file and demand one of the configuration's
image signatures (`utils::files::image_mime_in_bytes`), and the streaming path
also insists that the byte count the client declared in `X-Content-Size` matches
what arrived, so a body that stops short is removed rather than stored and
attached.

The progress request is a task of its own on the client, never a step the upload
waits for: Firefox does not resolve a stream response before its first byte, so
awaiting it means the file body is never sent at all. The totals stream ends by
itself in every case — its upload finishing, the upload never arriving (15 s), or
the upload stalling (30 s) — because a stream still open when the page navigates
is aborted by the browser, and dioxus turns an aborted request into a panic of
the whole client. An entry in the progress registry belongs to the upload that
`add_chunk` started, so a watcher never creates one.

The widget's `accept` list is extension tokens (`.jpg,.jpeg,.png,.gif,.webp`)
rather than the config's mime types: a browser takes either, but the window's
file dialog reads only extensions and would otherwise offer every file on disk.

Two things a deployment has to know:

- The uploads root (`UPLOAD_PATH`) needs its `tmp/`, `avatars/`, `artists/`
  and `releases/` directories. The Leptos `setup` used to create them; the
  dioxus server only reads `UPLOAD_PATH`, so `run-dioxus-gate.sh` now
  `mkdir -p`s them.
- Nothing in the transport caps a streamed body, so the per-configuration
  limit (10 MB avatars, 100 MB artist/release images) is enforced while
  streaming (413, partial file removed). A reverse proxy still needs its own
  body limit raised to match — nginx's `client_max_body_size` defaults to
  1 MB, which would reject these uploads before the app ever sees them.

Uploads have two paths, and both work in the browser modes. The reactive one
streams the file through the `/api/upload` server fn (with the progress bar) and
needs the client build above. The widget's markup is also a real
`multipart/form-data` form — `method="post"`, `enctype`, and hidden
`file_type`/`slug`/`artist_slug` fields ahead of the file part — so with
javascript off, or on the SSR-only server (which serves no wasm bundle), the
click is a browser form post to `/forms/upload` (`server::upload_form`), which
stores the file, attaches it to the row and 303s back with `?uploaded=1` or
`?bad_upload=<code>`. A file input in a form with neither POST nor multipart was
exactly what made uploads do nothing before slice 6c — the browser drops the file
and says so in the console. The javascript-off path has no progress bar: the
browser's own network UI is the feedback.

Attaching the file is deliberately narrow. The shared tail
(`services::files::upload`) writes **one column** — `primary_image` for an
artist or release, `avatar` for a user — because `Artist::update`,
`Release::update`, `Page::update`, `Track::update` and `RecordLabel::update`
all start by re-deriving `slug` from `name`, so saving the whole model to
attach a picture silently renames any row whose slug does not match its name
(that bug reached the gate: the `e2e-tabs` fixture lost its slug and every
admin check behind it served NotFound). The endpoint answers the stored name,
and pages that show the image pass `on_uploaded` to the widget and swap their
`<img src>` from it, which is what Leptos' reactive refresh looked like — a
page cannot refetch its own page data, since `use_page` never exposes its
resource.

### The desktop window (slices 6a–6b)

The same `src/ui` page tree also builds as a native window (`dioxus-desktop`:
a tao window around the system webview — on Linux that means `webkit2gtk-4.1`,
`gtk+-3.0` and `libsoup-3.0` must be installed). The easiest way to run it is
the dev launcher, which builds, stages the statics and opens the window:

```bash
sh end2end/run-dioxus-desktop.sh   # WHITE_LABEL_API_URL overrides the API (default 127.0.0.1:8127)
```

By hand that is:

```bash
dx build --platform desktop --bin dioxus-desktop --features desktop --no-default-features
cp -R public/. target/dx/dioxus-desktop/debug/linux/app/assets/
WHITE_LABEL_API_URL=http://127.0.0.1:8127 \
  ./target/dx/dioxus-desktop/debug/linux/app/dioxus-desktop
```

`cargo build --no-default-features --features desktop --bin dioxus-desktop`
compiles the same binary (handy for `cargo check`), but only `dx` gathers the
app's assets next to it, so run the copy under `target/dx`.

The `cp -R` is not decoration — `dx` puts only `asset!()`-registered files in a
native bundle, so without it the `assets/` directory is empty and the window
renders unstyled. The native resolver (`dioxus-asset-resolver::native`) reads
`/assets/<path>` from the executable's own directory, so `ui::assets`
(`bundled_asset`, `served_file_url`, `display_image_url`) gives bundled statics
the bundle address `/assets/main.css` on desktop while web and SSR keep
`/main.css`, and gives `/uploads/…` images the absolute API origin (plain `<img>`
subresources, so no CORS). Repeat the copy after every `dx build --platform
desktop` — the build recreates that directory; `dx bundle` will take the same
files from `[bundle] resources` in `Dioxus.toml` (slice 6f).

There is no SSR in this target: the app boots client-side and every server fn
in `ui/data.rs` is an HTTP call made by dioxus' own `reqwest` client, which
keeps a process-wide cookie jar, so the session cookie round-trips without any
CORS or `SameSite` work.

dioxus blocks in-app navigation after the first load, so the plain-HTML 303 form
fallbacks the SSR pages use are inert in the window: every form a user needs to
submit natively goes through the reactive `use_action` + JSON-twin path. Login,
register and logout joined them in slice 6d: three server fns under
`/api/client/*`, next to the `/auth/*` handlers rather than on the same paths
(the Leptos site still owns `POST /api/login` until Phase 8, and axum refuses a
path twice), answering an `AuthOutcome` — `LoggedIn { user }`, `Done`, or
`Failed { code }` with the same fixed codes the `?bad=` query carries.

Slice 6i finished the job for the rest of the app: the record-label, page,
artist, release, track and links editors plus the profile and change-password
pages each have a matching `/api/client/*` twin. They answer `EditOutcome` —
`Saved(row)`, which the page renders straight from (a rename, a soft delete, a
restore or an avatar change needs no reload), or `Failed { code }` carrying the
very code the 303 would have put in `?bad=`, so one alert table serves both
paths. The plain `<form action="/admin/forms/…">` post stays underneath as the
javascript-off path. The navbar learns what happened from the `ui::SessionHint`
context: the chrome outlives the navigation and a page resource resolves once per
component, so the menu renders the action's own answer instead of guessing
(dioxus ignores `key:` on a single node, so re-keying the menu would not have
rebuilt it).

The `server`-gated modules (router assembly, sessions, current-user
extractor, ported API endpoints) have their own tests, but plain `cargo test`
(and the pre-commit hook) only builds default features, so run them
explicitly:

```bash
cargo test --features server   # 539 lib tests, includes src/server/ + src/ui/ tests
cargo test --features server -- server::api   # just the API wire tests
cargo test --features server -- ui::          # just the route-table + UI component tests
```

## Running the project

```bash
cargo leptos watch
```

## Compiling for Release

```bash
cargo leptos build --release
```

Will generate your server binary in target/server/release and your site package in target/site

## Testing Your Project

```bash
cargo leptos end-to-end
```

```bash
cargo leptos end-to-end --release
```

Cargo-leptos uses Playwright as the end-to-end test tool.
Tests are located in end2end/tests directory.

### Running the tests with coverage

```bash
cargo watch -c -x test
```

### Running the tests with coverage

```bash
cargo watch -c -x "llvm-cov --html"
```

## Executing a Server on a Remote Machine Without the Toolchain

After running a `cargo leptos build --release` the minimum files needed are:

1. The server binary located in `target/server/release`
2. The `site` directory and all files within located in `target/site`

Copy these files to your remote server. The directory structure should be:

```text
white-label
site/
```

Set the following environment variables (updating for your project as needed):

```text
LEPTOS_OUTPUT_NAME="white-label"
LEPTOS_SITE_ROOT="site"
LEPTOS_SITE_PKG_DIR="pkg"
LEPTOS_SITE_ADDR="127.0.0.1:3000"
LEPTOS_RELOAD_PORT="3001"
```

Finally, run the server binary.
