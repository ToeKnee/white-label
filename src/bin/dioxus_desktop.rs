//! Desktop client entry (MIGRATION.md Phase 6 slice 6a).
//!
//! The same `crate::ui` page tree, launched in a system webview window
//! (`dioxus-desktop`: tao window + wry webview). There is no SSR here: the
//! app boots client-side and every fullstack call in `ui/data.rs` becomes an
//! HTTP request made by dioxus' own `reqwest` client, which keeps a
//! process-wide cookie jar, so the session cookie round-trips natively.
//!
//! Build and run:
//!
//! ```sh
//! dx build --platform desktop --bin dioxus-desktop --features desktop --no-default-features
//! WHITE_LABEL_API_URL=http://127.0.0.1:8127 ./target/dx/dioxus-desktop/debug/white-label
//! ```
//!
//! `WHITE_LABEL_API_URL` is the origin of the `dioxus-server` to talk to
//! (`IP`/`PORT` of the SSR server, e.g. the gate's 8127). Note that the
//! window's own origin is `http://dioxus.index.html/`, so only server fns
//! reach the API at this stage; server-hosted files (`/uploads/…`) get their
//! absolute URLs in slice 6b.

/// Origin of the API the window talks to. The leak is deliberate:
/// `set_server_url` takes a `&'static str` (it is set once per process, and
/// dioxus stores it in a `OnceLock`), and this config lives as long as the
/// process anyway.
fn api_origin() -> &'static str {
    match std::env::var("WHITE_LABEL_API_URL") {
        Ok(origin) => origin.leak(),
        Err(_) => "http://127.0.0.1:8127",
    }
}

fn main() {
    // Log to stdout like the server bin; `RUST_LOG` filters, as elsewhere.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let origin = api_origin();
    tracing::info!("desktop client talking to {origin}");
    dioxus::fullstack::set_server_url(origin);

    dioxus::launch(white_label::ui::App);
}
