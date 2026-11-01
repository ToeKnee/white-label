# White Label - a record label management website

> **Migration note:** The project is currently a Leptos 0.8 SSR app. A plan to migrate to
> Dioxus (web + desktop + iOS/Android from one codebase) is in [MIGRATION.md](MIGRATION.md).
> Until the migration lands, all notes below describe the Leptos setup.

## Migration checkpoint (branch `dioxus`)

A Dioxus server skeleton (Phase 1 of MIGRATION.md) lives in `src/server/` and
boots as a second binary; the shipped site stays on Leptos (`cargo leptos
watch`) until cutover:

```bash
npm install && npm run css   # tailwind + daisyUI -> public/main.css
cargo run --no-default-features --features server --bin dioxus-server
```

`IP`/`PORT` override the bind address. The `dx` CLI is pinned to the dioxus
crate version (currently 0.7.10); `dx serve` replaces the `cargo run` above
once Dioxus UI work (Phase 2+) lands.

The `server`-gated modules (router assembly, sessions, current-user
extractor) have their own tests, but plain `cargo test` (and the pre-commit
hook) only builds default features, so run them explicitly:

```bash
cargo test --features server   # 330 lib tests, includes src/server/ tests
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
