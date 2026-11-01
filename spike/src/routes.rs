//! UI for the spike. Markup follows the Leptos app's daisyUI structure
//! (navbar, prose articles, login fieldset) so rendered HTML can be compared.

use dioxus::prelude::*;
use dioxus_html::FileData;

use crate::api;

#[derive(Clone, Routable, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Route {
    #[layout(AppShell)]
    #[route("/")]
    Home {},
    #[route("/login")]
    Login {},
    #[route("/upload")]
    UploadDemo {},
    #[route("/artists")]
    Artists {},
    #[nest("/admin/artist/:artist_slug")]
    #[layout(ArtistAdmin)]
    #[route("/")]
    ArtistOverview { artist_slug: String },
    #[route("/releases/:release_slug")]
    ReleaseEdit {
        artist_slug: String,
        release_slug: String,
    },
    #[end_layout]
    #[end_nest]
    #[end_layout]
    #[route("/:..segments")]
    NotFound { segments: Vec<String> },
}

/// Page chrome: daisy navbar like the main app's `LabelHeader`, with a user
/// menu driven by the `get_user` server function.
///
/// Two dioxus-specific points vs the Leptos original:
/// - layouts receive no `children` prop; the child route renders via `Outlet`
///   (unlike Leptos' `parent_route!`).
/// - the user menu lives inside a `SuspenseBoundary` and reads its resource
///   via `suspend()`, which is what makes the SSR renderer await the server fn
///   and inline the resolved menu into the initial chunk.
#[component]
fn AppShell() -> Element {
    rsx! {
        div { class: "flex min-h-screen flex-col",
            div { class: "navbar bg-primary text-primary-content",
                div { class: "navbar-start",
                    Link { to: Route::Home {}, class: "btn btn-ghost text-xl", "Spike" }
                    Link { to: Route::Artists {}, class: "btn btn-ghost", "Artists" }
                }
                div { class: "navbar-end",
                    SuspenseBoundary {
                        fallback: |_cx: SuspenseContext| rsx! {
                            span { class: "animate-pulse px-4", "..." }
                        },
                        UserMenu {}
                    }
                }
            }
            main { class: "flex-1", Outlet::<Route> {} }
        }
    }
}

/// Navbar user menu: shows login state from `get_user` (server fn runs during
/// SSR thanks to `suspend()`; the session extractor sees the request cookie).
#[component]
fn UserMenu() -> Element {
    let mut user_resource = use_resource(api::get_user);
    let user = user_resource.suspend()?;

    match user() {
        Ok(Some(user)) => rsx! {
            div { class: "flex-none",
                ul { class: "menu menu-horizontal px-1",
                    li {
                        details {
                            summary { "{user.username}" }
                            ul { class: "rounded-t-none bg-base-100 p-2", "data-theme": "light",
                                li {
                                    Link { to: Route::UploadDemo {}, class: "btn btn-ghost",
                                        "Upload demo"
                                    }
                                }
                                li {
                                    button {
                                        class: "btn btn-ghost",
                                        onclick: move |_| async move {
                                            api::logout().await.ok();
                                            user_resource.restart();
                                        },
                                        "Log out"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
        Ok(None) => rsx! {
            span {
                Link { to: Route::Login {}, class: "btn btn-ghost", "Login" }
            }
        },
        Err(_) => rsx! {
            span { class: "px-4", "Session check failed" }
        },
    }
}

#[component]
fn Home() -> Element {
    rsx! {
        article { class: "prose my-6 md:container md:mx-auto",
            h1 { "Dioxus spike" }
            p {
                "Phase 0 of the Leptos to Dioxus migration: session auth, server functions, "
                "resources, nested routes and streaming uploads against the real backend."
            }
            SuspenseBoundary {
                fallback: |_cx: SuspenseContext| rsx! { p { class: "animate-pulse", "Loading..." } },
                HomeStatus {}
            }
        }
    }
}

/// Home page user state, resolved during SSR via `suspend()`.
#[component]
fn HomeStatus() -> Element {
    let user = use_resource(api::get_user).suspend()?;

    match user() {
        Ok(Some(user)) => rsx! {
            div { class: "card bg-base-200 shadow-xl",
                div { class: "card-body",
                    h2 { class: "card-title", "{user.username}" }
                    p { "id: {user.id}, email: {user.email}" }
                    div { class: "flex flex-wrap gap-2",
                        for permission in &user.permissions {
                            span { class: "badge badge-outline", "{permission}" }
                        }
                    }
                }
            }
        },
        Ok(None) => rsx! {
            p {
                "Not logged in. "
                Link { to: Route::Login {}, "Log in" }
                " to test session auth."
            }
        },
        Err(error) => rsx! {
            div { class: "alert alert-error", "Could not check the session: {error}" }
        },
    }
}

/// Artist listing via `list_artists`: proves `use_resource` runs during SSR
/// and gives the runtime checks a way to discover real slugs for the nested
/// admin routes.
#[component]
fn Artists() -> Element {
    rsx! {
        article { class: "prose my-6 md:container md:mx-auto",
            h1 { "Artists" }
            SuspenseBoundary {
                fallback: |_cx: SuspenseContext| rsx! { p { class: "animate-pulse", "Suspense fallback..." } },
                ArtistsList {}
            }
        }
    }
}

/// Inner component for the Suspense probe: `suspend()` is what registers the
/// pending resource with the SSR renderer (a plain read only yields `None`).
#[component]
fn ArtistsList() -> Element {
    let artists_resource = use_resource(api::list_artists);
    let artists = artists_resource.suspend()?;
    let artists = artists();
    let error_note: Option<String> = match &artists {
        Ok(_) => None,
        Err(error) => Some(error.to_string()),
    };
    let slugs: Vec<String> = match &artists {
        Ok(list) => list.iter().map(|artist| artist.slug.clone()).collect(),
        Err(_) => Vec::new(),
    };
    rsx! {
        if let Some(error_note) = error_note {
            div { class: "alert alert-error", "Could not load artists: {error_note}" }
        }
        ul { class: "menu bg-base-200 w-full max-w-md rounded-box",
            for slug in slugs {
                li {
                    Link {
                        to: Route::ArtistOverview { artist_slug: slug.clone() },
                        class: "w-full",
                        "{slug}"
                    }
                }
            }
        }
    }
}

/// Login page; form markup mirrors the main app's `components::auth::login`.
#[component]
fn Login() -> Element {
    let mut login_action = use_action(
        move |(username, password, remember): (String, String, Option<String>)| async move {
            api::login(username, password, remember).await
        },
    );
    let navigator = use_navigator();

    rsx! {
        article { class: "prose my-6 md:container md:mx-auto",
            h1 { "Login" }

            if let Some(Err(error)) = login_action.value() {
                div { class: "alert alert-error", "{error}" }
            }

            form {
                class: "grid gap-6",
                onsubmit: move |event| async move {
                    event.prevent_default();
                    let text_value = |name: &str| match event.get_first(name) {
                        Some(FormValue::Text(value)) => value,
                        _ => String::new(),
                    };
                    let username = text_value("username");
                    let password = text_value("password");
                    let remember = match event.get_first("remember") {
                        Some(FormValue::Text(value)) if !value.is_empty() => Some(value),
                        _ => None,
                    };
                    login_action.call((username, password, remember)).await;
                    if matches!(login_action.value(), Some(Ok(_))) {
                        navigator.replace(Route::Home {});
                    }
                },
                fieldset { class: "fieldset flex flex-col gap-6",
                    label { class: "input flex items-center gap-2",
                        svg {
                            xmlns: "http://www.w3.org/2000/svg",
                            view_box: "0 0 16 16",
                            fill: "currentColor",
                            class: "h-4 w-4 opacity-70",
                            path {
                                d: "M8 8a3 3 0 1 0 0-6 3 3 0 0 0 0 6ZM12.735 14c.618 0 1.093-.561.872-1.139a6.002 6.002 0 0 0-11.215 0c-.22.578.254 1.139.872 1.139h9.47Z",
                            }
                        }
                        input { r#type: "text", class: "grow", placeholder: "Username", name: "username" }
                    }

                    label { class: "input flex items-center gap-2",
                        svg {
                            xmlns: "http://www.w3.org/2000/svg",
                            view_box: "0 0 16 16",
                            fill: "currentColor",
                            class: "h-4 w-4 opacity-70",
                            path {
                                fill_rule: "evenodd",
                                clip_rule: "evenodd",
                                d: "M14 6a4 4 0 0 1-4.899 3.899l-1.955 1.955a.5.5 0 0 1-.353.146H5v1.5a.5.5 0 0 1-.5.5h-2a.5.5 0 0 1-.5-.5v-2.293a.5.5 0 0 1 .146-.353l3.955-3.955A4 4 0 1 1 14 6Zm-4-2a.75.75 0 0 0 0 1.5.5.5 0 0 1 .5.5.75.75 0 0 0 1.5 0 2 2 0 0 0-2-2Z",
                            }
                        }
                        input { r#type: "password", class: "grow", placeholder: "Password", name: "password" }
                    }

                    label { class: "label", r#for: "remember",
                        input {
                            r#type: "checkbox",
                            class: "checkbox",
                            id: "remember",
                            name: "remember",
                            checked: true,
                        }
                        "Remember me"
                    }

                    button { class: "btn btn-primary", r#type: "submit", "Login" }
                }
            }
        }
    }
}

/// Streaming upload + progress, the port of the app's upload widget.
#[component]
fn UploadDemo() -> Element {
    let mut file_type = use_signal(|| "artist_image".to_string());
    let mut slug = use_signal(|| "spike-test".to_string());
    let mut progress_bytes = use_signal(|| 0u64);
    let mut uploaded_bytes = use_signal(|| 0u64);
    let mut status = use_signal(String::new);

    let mut upload_action = use_action(
        move |(file_type, slug, file): (String, String, FileData)| async move {
            api::upload_file(file_type, slug, file.into()).await
        },
    );
    let mut progress_action = use_action(move |filename: String| async move {
        let mut stream = api::file_progress(filename).await?;
        while let Some(Ok(line)) = stream.next().await {
            if let Ok(total) = line.parse::<u64>() {
                progress_bytes.set(total);
            }
        }
        dioxus::Ok(())
    });

    rsx! {
        article { class: "prose my-6 md:container md:mx-auto",
            h1 { "Upload demo" }
            p {
                "Streaming multipart upload with two extra fields, plus a `TextStream` "
                "progress port of the app's FileProgress endpoint."
            }

            form {
                class: "flex flex-col gap-4",
                onsubmit: move |event| async move {
                    event.prevent_default();
                    status.set(String::new());
                    uploaded_bytes.set(0);
                    progress_bytes.set(0);

                    let Some(file) = event.files().into_iter().next() else {
                        status.set("Choose a file first.".to_string());
                        return;
                    };
                    let filename = file.name();

                    // Run the upload and the progress streamer concurrently;
                    // `call()` itself returns unit, results land in `value()`.
                    let upload_fut = upload_action.call((file_type(), slug(), file));
                    let progress_fut = progress_action.call(filename);
                    futures::future::join(upload_fut, progress_fut).await;

                    // `value()` hands back a live ReadSignal of the output,
                    // not the output itself - read it with a call.
                    match upload_action.value() {
                        Some(Ok(total_signal)) => {
                            let total = total_signal();
                            uploaded_bytes.set(total);
                            status.set(format!("Uploaded {total} bytes"));
                        }
                        Some(Err(error)) => status.set(format!("Upload failed: {error}")),
                        None => status.set("The upload did not report a result".to_string()),
                    }
                    if let Some(Err(error)) = progress_action.value() {
                        status.set(format!("{} (progress stream: {error})", status()));
                    }
                },
                div { class: "flex flex-wrap items-end gap-4",
                    label { class: "flex flex-col gap-1",
                        span { "Upload type" }
                        select {
                            class: "select",
                            name: "file_type",
                            value: "{file_type}",
                            onchange: move |event| file_type.set(event.value()),
                            option { value: "artist_image", "artist_image" }
                            option { value: "release_image", "release_image" }
                        }
                    }
                    label { class: "flex flex-col gap-1",
                        span { "Slug" }
                        input {
                            class: "input",
                            r#type: "text",
                            name: "slug",
                            value: "{slug}",
                            oninput: move |event| slug.set(event.value()),
                        }
                    }
                    label { class: "flex flex-col gap-1",
                        span { "File" }
                        input { class: "file-input", r#type: "file", name: "file" }
                    }
                    button { class: "btn btn-primary", r#type: "submit", "Upload" }
                }

                progress { class: "progress", value: "{progress_bytes}", max: "10485760" }
                p { "Progress: {progress_bytes} bytes. Finished: {uploaded_bytes} bytes. Cap: 10 MiB." }

                if !status.read().is_empty() {
                    div { class: "alert alert-info", "{status}" }
                }
            }
        }
    }
}

/// Layout for nested `/admin/artist/:artist_slug/...` routes; shows that the
/// nest-level param is available to the layout and to child routes (children
/// arrive through `Outlet`, not a prop).
#[component]
fn ArtistAdmin(artist_slug: String) -> Element {
    rsx! {
        div { class: "md:container md:mx-auto",
            nav { class: "tabs tabs-boxed my-4",
                Link {
                    to: Route::ArtistOverview { artist_slug: artist_slug.clone() },
                    class: "tab",
                    "Overview"
                }
            }
            Outlet::<Route> {}
        }
    }
}

/// Artist card via `use_resource` + `suspend()` inside a `SuspenseBoundary`
/// (proves the nest-level param reaches a server fn that the SSR renderer
/// awaits).
#[component]
fn ArtistOverview(artist_slug: String) -> Element {
    rsx! {
        article { class: "prose my-6",
            h1 { "Artist overview" }
            p { class: "text-base-content/60", "slug from nest: {artist_slug}" }
            SuspenseBoundary {
                fallback: |_cx: SuspenseContext| rsx! { p { class: "animate-pulse", "Loading..." } },
                ArtistCard { artist_slug }
            }
        }
    }
}

#[component]
fn ArtistCard(artist_slug: String) -> Element {
    let artist_resource = use_resource(move || api::get_artist(artist_slug.clone()));
    let artist = artist_resource.suspend()?;

    match artist() {
        Ok(artist) => rsx! {
            div { class: "card bg-base-200 shadow-xl",
                div { class: "card-body",
                    h2 { class: "card-title", "{artist.name}" }
                    p { {artist.description.unwrap_or_else(|| "No description set.".to_string())} }
                    p {
                        Link {
                            to: Route::ReleaseEdit {
                                artist_slug: artist.slug.clone(),
                                release_slug: "example-release".to_string(),
                            },
                            "Open a placeholder release edit page"
                        }
                    }
                }
            }
        },
        Err(error) => rsx! {
            div { class: "alert alert-error", "{error}" }
        },
    }
}

/// Leaf of the nested route: both the nest param and the leaf param arrive.
#[component]
fn ReleaseEdit(artist_slug: String, release_slug: String) -> Element {
    rsx! {
        article { class: "prose my-6",
            h1 { "Release edit" }
            p { "artist slug from nest: {artist_slug}" }
            p { "release slug from leaf: {release_slug}" }
            p {
                Link { to: Route::ArtistOverview { artist_slug }, "Back to overview" }
            }
        }
    }
}

#[component]
fn NotFound(segments: Vec<String>) -> Element {
    let path = segments.join("/");
    rsx! {
        article { class: "prose my-6 md:container md:mx-auto",
            h1 { "Not found" }
            p { "/{path}" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// Isolate router/layout rendering from the fullstack serve path: a plain
    /// VirtualDom with MemoryHistory must render the page component
    /// synchronously (no resource, no HTTP).
    #[test]
    fn page_components_render_synchronously_in_plain_vdom() {
        use std::rc::Rc;
        let mut dom = VirtualDom::new(|| {
            rsx! {
                dioxus::router::components::HistoryProvider {
                    history: move |_| {
                        Rc::new(dioxus::history::MemoryHistory::with_initial_path(
                            Route::Login {}.to_string(),
                        )) as Rc<dyn dioxus::history::History>
                    },
                    Router::<Route> {}
                }
            }
        });
        dom.rebuild_in_place();
        let html = dioxus_ssr::render(&dom);
        assert!(
            html.contains("Login"),
            "layout+page chain should render: {html}"
        );
        assert!(
            html.contains("Username"),
            "static form markup should render"
        );
    }

    #[test]
    fn top_level_routes_round_trip() {
        for (path, expected) in [
            ("/", Route::Home {}),
            ("/login", Route::Login {}),
            ("/upload", Route::UploadDemo {}),
            ("/artists", Route::Artists {}),
        ] {
            let parsed = Route::from_str(path).ok();
            assert_eq!(parsed, Some(expected.clone()), "{path:?} should parse");
            assert_eq!(expected.to_string(), path, "{path:?} should render back");
        }
    }

    #[test]
    fn nested_route_carries_both_params() {
        let path = "/admin/artist/tortoise/releases/tom";
        let parsed = Route::from_str(path).ok();
        assert_eq!(
            parsed,
            Some(Route::ReleaseEdit {
                artist_slug: "tortoise".to_string(),
                release_slug: "tom".to_string(),
            })
        );
        let rendered = parsed.map(|route| route.to_string());
        assert_eq!(rendered.as_deref(), Some(path));
    }

    /// Recorded behaviour (verified against dioxus-router 0.7.10): parsing
    /// tolerates a missing or extra trailing slash on both the nest base and
    /// the leaf, but rendering `ArtistOverview` canonically emits the trailing
    /// slash (the nest route is `/`), while `ReleaseEdit` renders without one.
    #[test]
    fn trailing_slashes_are_tolerant_when_parsing() {
        for path in ["/admin/artist/tortoise", "/admin/artist/tortoise/"] {
            assert_eq!(
                Route::from_str(path).ok(),
                Some(Route::ArtistOverview {
                    artist_slug: "tortoise".to_string(),
                }),
                "{path:?} should parse to the nest base"
            );
        }
        assert_eq!(
            Route::from_str("/admin/artist/tortoise/releases/tom/").ok(),
            Some(Route::ReleaseEdit {
                artist_slug: "tortoise".to_string(),
                release_slug: "tom".to_string(),
            })
        );
    }

    #[test]
    fn canonical_urls_for_nested_routes() {
        let overview = Route::ArtistOverview {
            artist_slug: "tortoise".to_string(),
        };
        assert_eq!(overview.to_string(), "/admin/artist/tortoise/");
        // The canonical form still parses back to the same route variant.
        assert_eq!(Route::from_str(&overview.to_string()).ok(), Some(overview));

        let edit = Route::ReleaseEdit {
            artist_slug: "tortoise".to_string(),
            release_slug: "tom".to_string(),
        };
        assert_eq!(edit.to_string(), "/admin/artist/tortoise/releases/tom");
        assert_eq!(Route::from_str(&edit.to_string()).ok(), Some(edit));
    }
}
