//! Desktop client entry (MIGRATION.md Phase 6 slice 6a, window metadata 6f).
//!
//! The same `crate::ui` page tree, launched in a system webview window
//! (`dioxus-desktop`: tao window + wry webview). There is no SSR here: the
//! app boots client-side and every fullstack call in `ui/data.rs` becomes an
//! HTTP request made by dioxus' own `reqwest` client, which keeps a
//! process-wide cookie jar, so the session cookie round-trips natively.
//!
//! The window's title and icon are set here rather than left to `dx`: the CLI
//! only bakes its `Dioxus.toml` title into the binary for release builds, so
//! setting them keeps the title correct for a plain `dx build` too.
//!
//! Build and run (the launcher also copies `public/` into the bundle, which
//! `dx` does not do for files that are not registered with `asset!()`):
//!
//! ```sh
//! sh end2end/run-dioxus-desktop.sh
//! ```
//!
//! Or by hand:
//!
//! ```sh
//! dx build --platform desktop --bin dioxus-desktop --features desktop --no-default-features
//! WHITE_LABEL_API_URL=http://127.0.0.1:8127 \
//!     ./target/dx/dioxus-desktop/debug/linux/app/dioxus-desktop
//! ```
//!
//! `WHITE_LABEL_API_URL` is the origin of the `dioxus-server` to talk to
//! (`IP`/`PORT` of the SSR server, e.g. the gate's 8127). Note that the
//! window's own origin is `http://dioxus.index.html/`, so links inside the app
//! have to be typed `Route`s (see `ui/assets.rs` for the equivalent rule about
//! files, and `MIGRATION.md` slice 6g for why plain `<a href>` escapes).

/// Title bar text, and the window class Linux desktop environments use to
/// match the taskbar entry to the `.desktop` file.
const WINDOW_TITLE: &str = "White Label";

/// The logo, compiled in so the window icon does not depend on the bundle's
/// asset layout. `public/Logo.png` is the same file the web and SSR targets
/// serve.
const LOGO_PNG: &[u8] = include_bytes!("../../public/Logo.png");

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

/// Window settings: title, a starting size that suits the admin tables, and
/// the app icon.
///
/// A logo that fails to decode is logged and ignored — the window still opens,
/// with dioxus' default icon. `tao`'s `Icon` wants raw RGBA, hence the decode.
fn window_config() -> dioxus::desktop::Config {
    let window = dioxus::desktop::WindowBuilder::new()
        .with_title(WINDOW_TITLE)
        .with_inner_size(dioxus::desktop::LogicalSize::new(1280.0, 800.0));
    let config = dioxus::desktop::Config::new().with_window(window);

    match dioxus::desktop::icon_from_memory::<dioxus::desktop::tao::window::Icon>(LOGO_PNG) {
        Ok(icon) => config.with_icon(icon),
        Err(problem) => {
            tracing::warn!("window icon not applied: {problem}");
            config
        }
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

    dioxus::LaunchBuilder::new()
        .with_cfg(window_config())
        .launch(white_label::ui::App);
}
