# White Label - a record label management website

> **Status:** the app is now the Dioxus stack (branch `dioxus`, in
> [MIGRATION.md](MIGRATION.md)): an axum server with Dioxus SSR pages, a wasm
> client that hydrates them, and a desktop webview window — one codebase, with
> iOS/Android still to come (Phase 7). The old Leptos site is gone. Notes below
> describe the Dioxus setup.

## Migration checkpoint (branch `dioxus`)

The Dioxus app lives in
`src/server/` (axum router: sessions, uploads, current-user extractor, the
no-JS form handlers under `/auth/*`, `/admin/forms/*` and `/forms/upload`,
the JSON server fns under `/api/*`, and the public-dir and `/uploads` static
mounts; uploads finish through the shared
`services::files::upload` tail) and `src/ui/` (the whole page tree: route
table, layouts, header/footer chrome, public pages, auth pages, the admin
dashboard and the artist/release/track/links/images editors). The old Leptos
default binary is deleted, so this _is_ the app now:

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
does the build-and-serve pair for manual work. With the Leptos stack deleted
(Phase 8) the bundle is pure Dioxus, and a debug bundle is still big
(~25 MB of wasm): the whole app plus the dioxus runtime, compiled
unoptimised — `dx build --release` is the shape that ships.

A debug wasm bundle always tries one websocket we do not serve: `ws://<host
the page came from>/_dioxus`, dioxus-web's hot-patch channel (`devtools::init`,
gated `all(feature = "devtools", debug_assertions)`), which only `dx serve`
answers. Firefox logs one failed-connection line per attempt and the client
backs off to 4 s between tries; nothing in the app depends on it, and a release
bundle (`--prof`) does not contain the code at all. Serving that path ourselves
would mean an unauthenticated websocket endpoint for no functional gain, so the
axum router deliberately does not have one.

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

`cargo test` covers the SSR side; the hydrated client side
is covered by `end2end/tests/dioxus_client.spec.ts` (SPA nav issues no extra
document loads, the links editor submits without a round trip, login and logout
submit through the `/api/login` and `/api/logout` twins so the navbar follows
the session
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

### The desktop window (slices 6a–6f)

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
desktop` — the build recreates that directory.

`dx bundle` cannot carry that tree today, and it is worth knowing why before
Phase 7 tries it: `[bundle] resources` entries are copied **file by file, flat**
into `usr/lib/<product name>/` (only the file name is kept, and a directory
entry fails the bundle), while the window asks for `/assets/images/…` with its
subdirectories. `dx` does recurse into `asset_dir` (`assets/` by default) and
maps it to `assets/…` in the bundle, which is exactly the shape the resolver
wants — so when real packaging matters the options are to move the statics
under `assets/`, register them with `asset!()`, or serve them from the API
origin instead of the bundle.

The window's title (`White Label`) and icon come from code, not `Dioxus.toml`:
0.7.10 has no `[desktop]` config section, and the CLI only bakes `[web.app]
title` into release builds, so a plain `dx build` window would otherwise be
called "Dioxus App". `src/bin/dioxus_desktop.rs` passes a
`dioxus::desktop::Config` (title, 1280x800 starting size) through
`dioxus::LaunchBuilder::with_cfg`, and decodes `public/Logo.png` — compiled in
with `include_bytes!`, so the icon does not depend on the bundle layout — into a
`tao` RGBA icon; a decode failure is logged and the default icon is used. The
`[bundle]` table in `Dioxus.toml` (identifier, publisher, descriptions,
copyright, `icon = [Logo.png, Logo.svg]`) is separate: those are the values
`dx bundle` puts in the Linux `.desktop` entry and hicolor theme, and the
identifier is the permanent store identity on Android/iOS. A first
`dx bundle --platform desktop --package-types deb` (phase 8) proved the table
and surfaced two quirks: `dx`'s `AppCategory` parser does not accept `Office`
(it silently emitted an empty `Categories=`; the accepted spelling that maps
to freedesktop `Office;` is `Business`), and the launcher names come from the
bin (`Package: dioxus-desktop`, `Name=DioxusDesktop`), not `[application]`'s
`name`.

There is no SSR in this target: the app boots client-side and every server fn
in `ui/data.rs` is a dioxus server fn that becomes an HTTP call, made by
dioxus' own `reqwest` client, which keeps a process-wide cookie jar, so the
session cookie round-trips without any CORS or `SameSite` work.

dioxus blocks in-app navigation after the first load, so the plain-HTML 303 form
fallbacks the SSR pages use are inert in the window: every form a user needs to
submit natively goes through the reactive `use_action` + JSON-twin path. Login,
register and logout joined them in slice 6d: three server fns under
`/api/login`, `/api/register` and `/api/logout`, next to the `/auth/*`
handlers rather than on the same path
(axum refuses a method+path twice), answering an `AuthOutcome` —
`LoggedIn { user }`, `Done`, or `Failed { code }` with the same fixed codes the
`?bad=` query carries.

Slice 6i finished the job for the rest of the app: the record-label, page,
artist, release, track and links editors plus the profile and change-password
pages each have a matching JSON twin under `/api/*` (e.g. `/api/update_artist`,
`/api/restore_track`). They answer `EditOutcome` —
`Saved(row)`, which the page renders straight from (a rename, a soft delete, a
restore or an avatar change needs no reload), or `Failed { code }` carrying the
very code the 303 would have put in `?bad=`, so one alert table serves both
paths. The plain `<form action="/admin/forms/…">` post stays underneath as the
javascript-off path. The navbar learns what happened from the `ui::SessionHint`
context: the chrome outlives the navigation and a page resource resolves once per
component, so the menu renders the action's own answer instead of guessing
(dioxus ignores `key:` on a single node, so re-keying the menu would not have
rebuilt it).

The server-gated code (router assembly, sessions, current-user extractor,
form handlers and server fns) and the whole `src/ui` suite build under the
default `server` feature, so one command covers everything:

```bash
cargo test            # 478 lib tests + doc tests, src/server/ and src/ui/ included
cargo test -- server::   # just the router/form-handler tests
cargo test -- ui::       # just the route-table + UI component tests
```

## Running the project

The dev flow is build-and-serve (there is no `dx serve` web watch mode for
this crate — the note above and MIGRATION.md risk 10 explain why):

```bash
sh end2end/run-dioxus-web.sh      # wasm client, served hydrated on 127.0.0.1:8131
sh end2end/run-dioxus-desktop.sh  # window against the API on 127.0.0.1:8127
```

SSR-only (plain-HTML forms, full-reload navigation):

```bash
IP=127.0.0.1 PORT=8127 DIOXUS_PUBLIC_PATH=public \
  cargo run --bin dioxus-server
```

## Compiling for Release

```bash
npm install && npm run css        # public/main.css, picked up by the web bundle
cargo build --release --bin dioxus-server
dx build --release --platform web --bin dioxus-web --features web --no-default-features
```

The `dx` web bundle lands in `target/dx/dioxus-web/release/web/public`; the
release deploys as that bundle plus the server binary, with
`DIOXUS_PUBLIC_PATH` pointing at the bundle directory. Desktop packaging
works: `dx bundle --platform desktop --package-types deb --bin dioxus-desktop
--features desktop --no-default-features --release` produces an installable
`.deb` under `target/dx/dioxus-desktop/bundle/linux/deb/` (proven in phase 8;
the statics gap noted in the desktop section still applies). Mobile
packaging is Phase 7 work.

Dependency policy: `Cargo.lock` is committed (this crate ships as binaries)
and CI/release builds pass `--locked` — `dx build` and `dx bundle` accept it
as `--cargo-args --locked`. A lockfile that is stale or missing should fail
the build rather than silently re-resolve. Dependency updates mean
regenerating the lockfile in its own commit and rerunning the gate. (Fresh
lock-free resolution works since the yanked `axum_session` stack left with
Leptos — but builds should not depend on that; see MIGRATION.md risk 9.)

## Testing Your Project

```bash
cargo test                       # unit + doc tests (default features = server)
sh end2end/run-dioxus-gate.sh    # curl pins + Playwright (chromium & firefox)
```

Playwright is the end-to-end tool; tests are in `end2end/tests/`. The gate
needs the dev database and the Playwright 1.44.1 browser builds (once:
`npx playwright install chromium firefox` inside `end2end/`); it prints
`== GATE OK ==` when every pin passes.

### Running the tests with coverage

```bash
cargo llvm-cov --html            # needs `cargo install cargo-llvm-cov`
```

## Executing a Server on a Remote Machine Without the Toolchain

After the release build above, the minimum files needed are:

1. The server binary (`target/release/dioxus-server`)
2. The web bundle directory (`target/dx/dioxus-web/release/web/public`)

Copy these files to your remote server. The directory structure should be:

```text
dioxus-server
public/
```

Set the following environment variables (updating for your project as needed):

```text
IP=0.0.0.0
PORT=8080
DIOXUS_PUBLIC_PATH=public
DATABASE_URL=postgres://user:pass@localhost/whitelabel
UPLOAD_PATH=/var/lib/white-label/uploads
```

`UPLOAD_PATH` needs its `tmp/`, `avatars/`, `artists/` and `releases/`
directories, and any reverse proxy in front needs a body limit that matches
the upload sizes (see the uploads note above). Then run the server binary.
