# White Label - a record label management website

> **Migration note:** The project is currently a Leptos 0.8 SSR app. A plan to migrate to
> Dioxus (web + desktop + iOS/Android from one codebase) is in [MIGRATION.md](MIGRATION.md).
> Until the migration lands, all notes below describe the Leptos setup.

## Migration checkpoint (branch `dioxus`)

The Dioxus port (Phases 1–5 of MIGRATION.md) lives in
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
8127 (SSR-only, `public/`) and 8131 (hydrated). `dx serve --platform web
--bin dioxus-web --features web --no-default-features` is the watch-mode
equivalent; expect it to also compile a native "server build" of the crate,
which is harmless here and ignored. Debug bundles are large (~39 MB) because
the Leptos modules (`src/routes`, `src/components`) are still compiled
unconditionally — cfg-gating them out of `web` builds is a Phase 8 cleanup
item.

`cargo test --features server` covers the SSR side; the hydrated client side
is covered by `end2end/tests/dioxus_client.spec.ts` (SPA nav issues no extra
document loads, the links editor submits without a round trip, the profile
avatar upload and the artist image upload both run through the server fns
with a progress bar) — it needs the
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

(`end2end`'s Playwright is 1.44.1; run `npx playwright install chromium` in
`end2end/` once if the `chromium-1117` browser build is missing.)

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

The upload is javascript-only by design: the protocol has no form-post twin, so
SSR renders the picker and the button, and uploading needs the client build
above.

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

The `server`-gated modules (router assembly, sessions, current-user
extractor, ported API endpoints) have their own tests, but plain `cargo test`
(and the pre-commit hook) only builds default features, so run them
explicitly:

```bash
cargo test --features server   # 490 lib tests, includes src/server/ + src/ui/ tests
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
