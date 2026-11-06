//! The app root and shared page chrome.
//!
//! The fullstack renderer owns the `html`/`head`/`body` wrapper, `App` adds
//! the head elements, and the `AppShell` layout supplies the navbar /
//! `<main>` / footer around every page.

// Dioxus component functions are conventionally PascalCase, and `Element` is
// a type alias for a `Result`, which trips pedantic lints that do not apply
// to components (`App` never returns an error variant).
#![expect(
    non_snake_case,
    reason = "dioxus root/layout components are PascalCase by convention and are referenced as `App`/`AppShell` from the router and server"
)]
#![expect(
    clippy::missing_errors_doc,
    reason = "`Element` is a `Result` alias by type, not by API: components never return the error variant; rendering failures surface in the framework's own error handling"
)]

use dioxus::prelude::*;

use crate::ui::GlobalState;
use crate::ui::Route;
use crate::ui::SessionHint;
use crate::ui::assets::bundled_asset;
use crate::ui::footer::LabelFooter;
use crate::ui::header::LabelHeader;

/// The application root — handed to `dioxus::server::router` in
/// `crate::server::build_router`.
///
/// Provides the `GlobalState` context and sets up the document head. (The
/// current user is not a context: every component that needs it reads the
/// session through `super::use_user`.)
///
/// The session hint is the exception: it is what a reactive sign-in or
/// sign-out hands the chrome, which outlives the navigation and cannot
/// re-read its own page resource (see [`SessionHint`]).
pub fn App() -> Element {
    use_context_provider(|| Signal::new(GlobalState::default()));
    use_context_provider(|| Signal::new(SessionHint::Unknown));

    rsx! {
        document::Meta { charset: "utf-8" }
        document::Meta {
            name: "viewport",
            content: "width=device-width, initial-scale=1",
        }
        document::Title { "White Label" }
        // Tailwind + daisyUI build (`npm run css`). The server mounts
        // `public/main.css` at this exact path, ahead of the SSR fallback; a
        // desktop window loads it from its own bundle instead (slice 6b).
        document::Stylesheet { href: bundled_asset("/main.css") }
        Router::<Route> {}
    }
}

/// Chrome shared by every route: navbar, page body inside `<main>`, footer.
/// (Leptos: `Router > LabelHeader`, `main > Routes`, `LabelFooter`.)
#[component]
pub fn AppShell() -> Element {
    rsx! {
        LabelHeader {}
        main { Outlet::<Route> {} }
        LabelFooter {}
    }
}
