//! Dioxus migration server (MIGRATION.md Phases 1–3: router, API port and
//! the `crate::ui` app shell with placeholder pages). The shipped site is
//! still the default bin (`src/main.rs`, Leptos) until cutover.
//!
//! Run manually:
//!
//! ```sh
//! IP=127.0.0.1 PORT=8127 DIOXUS_PUBLIC_PATH=public \
//!   cargo run --no-default-features --features server --bin dioxus-server
//! ```
//!
//! `dx serve` sets those variables itself once Dioxus UI work starts.

fn main() {
    // `dioxus::serve` owns the tokio runtime and reads IP/PORT.
    dioxus::serve(|| async {
        let _ = dotenvy::dotenv();
        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
            )
            .try_init();

        white_label::server::build_router(
            white_label::database::create_pool().await,
            std::env::var("UPLOAD_PATH")
                .ok()
                .map(std::path::PathBuf::from),
        )
        .await
    });
}
