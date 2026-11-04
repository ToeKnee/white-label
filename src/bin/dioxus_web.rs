//! Web (wasm) client entry (MIGRATION.md Phase 5). Built to
//! `wasm32-unknown-unknown` by `dx build`; the SSR server injects the
//! bundle into every page when the public dir it serves contains the
//! generated `index.html` (see `dioxus_server::ServeConfig`).
//!
//! Build manually:
//!
//! ```sh
//! dx build --platform web --bin dioxus-web --features web --no-default-features
//! ```
//!
//! On the client the fullstack macros in `ui/data.rs` become HTTP calls to
//! the `/api/*` server fns; the server side of those fns lives in the
//! `dioxus-server` bin.

fn main() {
    dioxus::launch(white_label::ui::App);
}
