# White Label - a record label management website

> **Status:** one Dioxus codebase with three surfaces: an axum server that
> serves Dioxus SSR pages, a wasm client that hydrates them, and a desktop
> webview window. iOS/Android packaging is still to come. Notes below
> describe the current setup.

## How the app fits together

The Dioxus app lives in
`src/server/` (axum router: sessions, uploads, current-user extractor, the
no-JS form handlers under `/auth/*`, `/admin/forms/*` and `/forms/upload`,
the JSON server fns under `/api/*`, and the public-dir and `/uploads` static
mounts; uploads finish through the shared
`services::files::upload` tail) and `src/ui/` (the whole page tree: route
table, layouts, header/footer chrome, public pages, auth pages, the admin
dashboard and the artist/release/track/links/images editors). The three
`dioxus-*` bins are the whole app now:

```bash
npm install   # tailwind + daisyUI CLI; `build.rs` then regenerates public/main.css
cargo run --no-default-features --features server --bin dioxus-server
```

`IP`/`PORT` override the bind address; `DIOXUS_PUBLIC_PATH=public` makes the
server serve `public/main.css`, the favicon and the images (dioxus mounts
each top-level public-dir file at its own path). `public/main.css` is
generated (gitignored): tailwind generates utilities by scanning the
sources, so the crate's `build.rs` reruns `npm run css` whenever `src/`,
`style/` or the npm config changes — the stylesheet can no longer lag
behind the classes the code uses. A machine without `node_modules` (or
without `npm` on PATH) skips that step with a `cargo:warning`; prebuild
`public/main.css` with `npm run css` there before serving. The `dx` CLI is pinned to
the dioxus crate version (currently 0.7.10).

That SSR-only shape serves **no wasm bundle**: the pages render, navigate with
full reloads, log in and save through the plain-HTML forms, but the reactive
features — including every upload (see the uploads note below) — need the
client build. `sh end2end/run-dioxus-web.sh` builds it and serves the hydrated
site, which is the shape the deployed site will have.

### The wasm client

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
does the build-and-serve pair for manual work. The bundle is pure Dioxus, and
a debug bundle is still big
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
lists). Neither has landed; until one does, the build-and-serve commands above
are the dev flow.

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

### Uploads with progress

`src/ui/upload.rs` holds the picker widget and its two dioxus endpoints. It is
rendered on the profile page (`/profile`, the signed-in user's avatar), the
admin artist **Images** tab (`/admin/artist/<slug>/images`), the release editor
(`EditRelease`) and the track editor (`EditTrack`, which uploads the track's own
audio and plays it back with an `<audio controls>` element — the same file the
public release page plays):

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
invented by dioxus from a table of _web asset_ types that holds no images and no
audio at all (a `.png` arrives as `text/html; charset=utf-8`), so a claim the
configuration does not accept falls back to the file's extension
(`utils::files::claimed_upload_mime`) — which also covers a browser's own legacy
spellings (`audio/x-flac` for a `.flac`, say). Both upload paths then read back
the first bytes of the temp file and demand one of the configuration's image or
audio signatures (`utils::files::image_mime_in_bytes` and
`utils::files::audio_mime_in_bytes`), and the streaming path also insists that
the byte count the client declared in `X-Content-Size` matches what arrived, so
a body that stops short is removed rather than stored and attached.

The track audio slot (`file_type=TrackAudio`) takes `audio/mpeg`, `audio/wav`,
`audio/flac` and `audio/ogg` — the four every target browser plays natively.
`.m4a`/`.aac` are left out on purpose: their `ftyp` box carries a brand field
rather than a fixed signature, and Safari is the only browser that needs them.

The progress request is a task of its own on the client, never a step the upload
waits for: Firefox does not resolve a stream response before its first byte, so
awaiting it means the file body is never sent at all. The totals stream ends by
itself in every case — its upload finishing, the upload never arriving (15 s), or
the upload stalling (30 s) — because a stream still open when the page navigates
is aborted by the browser, and dioxus turns an aborted request into a panic of
the whole client. An entry in the progress registry belongs to the upload that
`add_chunk` started, so a watcher never creates one.

The widget's `accept` list is extension tokens (`.jpg,.jpeg,.png,.gif,.webp`, or
`.mp3,.wav,.flac,.ogg` for the audio slot) rather than the config's mime types: a
browser takes either, but the window's file dialog reads only extensions and
would otherwise offer every file on disk.

Two things a deployment has to know:

- The uploads root (`UPLOAD_PATH`) needs its `tmp/`, `avatars/`, `artists/`,
  `releases/` and `tracks/` directories. The dioxus server only reads
  `UPLOAD_PATH` and never creates them, so deployments must `mkdir -p` them
  (`run-dioxus-gate.sh` does).
- Nothing in the transport caps a streamed body, so the per-configuration
  limit (10 MB avatars, 100 MB artist/release images, 250 MB track audio) is
  enforced while streaming (413, partial file removed). A reverse proxy still
  needs its own body limit raised to match — nginx's `client_max_body_size`
  defaults to 1 MB, which would reject these uploads before the app ever sees
  them.

Uploads have two paths, and both work in the browser modes. The reactive one
streams the file through the `/api/upload` server fn (with the progress bar) and
needs the client build above. The widget's markup is also a real
`multipart/form-data` form — `method="post"`, `enctype`, and hidden
`file_type`/`slug` fields ahead of the file part (`artist_slug` for a release or
track, `release_slug` also for a track, since those editors are addressed by
more than one slug) — so with
javascript off, or on the SSR-only server (which serves no wasm bundle), the
click is a browser form post to `/forms/upload` (`server::upload_form`), which
stores the file, attaches it to the row and 303s back with `?uploaded=1` or
`?bad_upload=<code>`. A file input in a form with neither POST nor multipart is
exactly what made uploads do nothing during development — the browser drops the
file and says so in the console. The javascript-off path has no progress bar: the
browser's own network UI is the feedback.

Attaching the file is deliberately narrow. The shared tail
(`services::files::upload`) writes **one column** — `primary_image` for an
artist, release or track, `avatar` for a user, `audio` for a track — because
`Artist::update`,
`Release::update`, `Page::update`, `Track::update` and `RecordLabel::update`
all start by re-deriving `slug` from `name`, so saving the whole model to
attach a picture silently renames any row whose slug does not match its name
(that bug reached the gate: the `e2e-tabs` fixture lost its slug and every
admin check behind it served NotFound). The endpoint answers the stored name,
and pages that show the image pass `on_uploaded` to the widget and swap their
`<img src>` from it — a page cannot refetch its own page data, since `use_page`
never exposes its resource.

Uploads also get garbage collection at the row end: the admin "Housekeeping"
card (and `services::purge`) permanently removes rows past their 90-day
soft-delete window, and each upload file that no surviving row names any more —
a name only the doomed rows held, live rows included when a purge cascades
them — is named in the preview (the card's "Upload files" listing, `<dir>/<file>`
under `UPLOAD_PATH`) and deleted after the commit. A name any surviving row
still shares (two tracks on one stored audio file), an artist the purge had to
skip, or a restorable row inside its window keeps its files; a hand-edited name
with a path separator is never offered and never removed; and without a
configured `UPLOAD_PATH` the purge only removes rows. The sweep goes further
and collects files the database stopped naming long ago — replaced
avatars/covers/audio leave their previous file behind, and purges predating
this service left rows' files on disk — anything under `UPLOAD_PATH`'s
`artists`, `releases`, `tracks` or `avatars` directories that no row names at
all is previewed and removed, and such a file-only purge is worth running even
with no rows past their window. The `tmp/` staging directory and stray
subdirectories are never swept.

### The desktop window

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
mobile packaging leans on it: `[bundle] resources` entries are copied **file by file, flat**
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
`dx bundle --platform desktop --package-types deb` proved the table
and surfaced two quirks: `dx`'s `AppCategory` parser does not accept `Office`
(it silently emitted an empty `Categories=`; the accepted spelling that maps
to freedesktop `Office;` is `Business`), and the launcher names come from the
bin (`Package: dioxus-desktop`, `Name=DioxusDesktop`), not `[application]`'s
`name`.

There is no SSR in this target: the app boots client-side and every server fn
in `ui/data.rs` is a dioxus server fn that becomes an HTTP call, made by
dioxus' own `reqwest` client, which keeps a process-wide cookie jar, so the
session cookie round-trips without any CORS or `SameSite` work.

That jar now outlives the window when the user asked it to. On the web the
browser does the remembering: "Remember me" (checked by default on the login
form) gives the session cookie a 30-day `Max-Age`
(`server::current_user::remember_session`) and the browser persists it. On
desktop `src/session_store.rs` + `src/session_store/desktop.rs` do the same
with the OS credential store (`keyring`'s `v1` mode, MIT/Apache-2.0: Keychain
on macOS, Credential Manager on Windows, the Secret Service — GNOME Keyring,
KWallet — on Linux). The bin's `prepare()` (called in `dioxus_desktop.rs`
after `set_server_url`, before launch) replaces dioxus' lazily-built client
through its `GLOBAL_REQUEST_CLIENT` hook with one whose cookie jar captures
full `Set-Cookie` lines and re-seeds from the store at startup. A cookie that
arrives with a positive `Max-Age` is stored verbatim (attributes and all) under
service `net.ynottony.whitelabel`, one entry per API origin; the next start
ages it down to the time left and stops accepting it once it is past, exactly
like a browser would, and logout or expiry deletes the entry. Session cookies
without `Max-Age` — the ones that should die with the window — are never
stored. A machine with no Secret Service at all gets a warning and an ordinary
non-remembered login. The decision rules (`decide`, `restore_cookie`) are
framework-free and unit-tested under the server build; the window itself is
never exercised in CI.

dioxus blocks in-app navigation after the first load, so the plain-HTML 303 form
fallbacks the SSR pages use are inert in the window: every form a user needs to
submit natively goes through the reactive `use_action` + JSON-twin path. Login,
register and logout joined them: three server fns under
`/api/login`, `/api/register` and `/api/logout`, next to the `/auth/*`
handlers rather than on the same path
(axum refuses a method+path twice), answering an `AuthOutcome` —
`LoggedIn { user }`, `Done`, or `Failed { code }` with the same fixed codes the
`?bad=` query carries.

The same reactive path covers the rest of the app: the record-label, page,
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
cargo test            # 503 lib tests + doc tests, src/server/ and src/ui/ included
cargo test -- server::   # just the router/form-handler tests
cargo test -- ui::       # just the route-table + UI component tests
```

## Running the project

The dev flow is build-and-serve (there is no `dx serve` web watch mode for
this crate — the wasm client note above explains why):

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
npm install                     # `build.rs` regenerates public/main.css from here on
cargo build --release --bin dioxus-server
dx build --release --platform web --bin dioxus-web --features web --no-default-features
```

The `dx` web bundle lands in `target/dx/dioxus-web/release/web/public`; the
release deploys as that bundle plus the server binary, with
`DIOXUS_PUBLIC_PATH` pointing at the bundle directory. Desktop packaging
works: `dx bundle --platform desktop --package-types deb --bin dioxus-desktop
--features desktop --no-default-features --release` produces an installable
`.deb` under `target/dx/dioxus-desktop/bundle/linux/deb/` (proven;
the statics gap noted in the desktop section still applies). Mobile
packaging (iOS/Android) is the remaining target, and its open items are
tracked in `Ideas.md`.
Desktop and mobile render through the system webview (wry): the same
HTML/CSS everywhere and no native widgets — an accepted design choice.

Dependency policy: `Cargo.lock` is committed (this crate ships binaries)
and CI/release builds pass `--locked` — `dx build` and `dx bundle` accept it
as `--cargo-args --locked`. A lockfile that is stale or missing should fail
the build rather than silently re-resolve. Dependency updates mean
regenerating the lockfile in its own commit and rerunning the gate. Keep
the `dx` CLI and the `dioxus` crates on the same 0.7.x (0.7.10 at the
moment); re-evaluate when 0.8 goes stable.

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

`UPLOAD_PATH` needs its `tmp/`, `avatars/`, `artists/`, `releases/` and
`tracks/` directories, and any reverse proxy in front needs a body limit that
matches the upload sizes (see the uploads note above). Then run the server
binary.
