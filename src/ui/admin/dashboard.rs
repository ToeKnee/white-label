//! The admin dashboard: a card row, each card behind its own
//! `SuspenseBoundary`.
//!
//! The row covers label summary, artists table, pages table, social tips,
//! upcoming-release countdown and mailing-list stub, so one slow load
//! renders its fallback without blocking the rest.
//!
//! Two deliberate choices:
//!
//! - The countdown is a snapshot of `Utc::now()` at render (SSR only, so no
//!   `set_interval` tick yet; the values land when the client builds come).
//! - Social tips rotate daily — the index comes from the date, so SSR and
//!   client renders agree on which tip shows.
//!
//! The Housekeeping card at the bottom runs the purge of rows past their
//! 90-day soft-delete window; see [`crate::services::purge`] for the rules
//! and `PurgeCard` below for how each path (form posts, reactive twin)
//! reports itself.

use chrono::{DateTime, Datelike, Utc};
use dioxus::prelude::*;

use crate::api_types::edit::EditOutcome;
use crate::api_types::purge::{PURGE_AFTER_DAYS, PurgeResult};
use crate::models::artist::Artist;
use crate::models::page::Page;
use crate::models::record_label::RecordLabel;
use crate::models::release::Release;
use crate::ui::Route;
use crate::ui::data::{
    get_label_artists, get_label_pages, get_next_scheduled_release, get_purge_preview,
};
use crate::ui::page_data;
use crate::ui::reactive::{refused_code, saved_row, transport_message};
use crate::ui::use_label;
use crate::ui::use_page;
use crate::ui::use_user;
use crate::ui::utils::{InlineError, Loading, StatusBadge, Success};
use crate::utils::shorten_string::shorten_string;
use dioxus::router::Link;

// Server-only: the twin's body runs in the server half of the `#[post]`
// macro, next to the same service the javascript-off handler calls.
#[cfg(feature = "server")]
use crate::server::forms::purge_code;
#[cfg(feature = "server")]
use crate::services;

/// The dashboard: the full card row, plus the housekeeping card below it.
#[component]
pub fn Dashboard(
    /// Rows a purge preview found, as a numeric string (form redirect).
    ready: Option<String>,
    /// Rows a purge removed, as a numeric string (form redirect).
    purged: Option<String>,
    /// Past-window artists a ran purge left in place (form redirect).
    skipped: Option<String>,
    /// Fixed code from a refused purge (form redirect).
    bad: Option<String>,
) -> Element {
    rsx! {
        document::Title { "Admin Dashboard" }
        div { class: "flex flex-row flex-wrap gap-6 justify-between",
            RecordLabelCard {}
            ArtistsTable {}
            PagesTable {}
            SocialMediaTips {}
            UpcomingRelease {}
            MailingListCard {}
        }
        PurgeCard { ready, purged, skipped, bad }
    }
}

/// Quick label info for `label_owner` sessions (nothing for other users).
#[component]
fn RecordLabelCard() -> Element {
    rsx! {
        SuspenseBoundary {
            fallback: |_context: SuspenseContext| rsx! { Loading {} },
            RecordLabelCardBody {}
        }
    }
}

#[component]
fn RecordLabelCardBody() -> Element {
    let session = use_user()?;
    let is_owner = session()
        .ok()
        .flatten()
        .is_some_and(|user| user.permissions.contains("label_owner"));
    if !is_owner {
        return rsx! {};
    }

    let label_result = use_label()?;
    // A failed label load renders the empty defaults.
    let label = match label_result() {
        Ok(result) => result.record_label,
        Err(error) => {
            tracing::error!("Error while getting label for dashboard: {error}");
            RecordLabel::default()
        }
    };
    let description_summary = shorten_string(label.description.clone());

    rsx! {
        div { class: "overflow-x-auto shadow-xl basis-sm not-prose card bg-neutral text-neutral-content bg-base-100",
            div { class: "card-body",
                h2 { class: "card-title", "{label.name}" }
                p { "ISRC: {label.isrc_base} YY XXXXX" }
                p { "{description_summary}" }
                div { class: "justify-end card-actions",
                    Link {
                        to: Route::EditLabel { updated: None, bad: None },
                        class: "btn btn-primary",
                        "Edit"
                    }
                }
            }
        }
    }
}

/// The label's artists table.
#[component]
fn ArtistsTable() -> Element {
    rsx! {
        div { class: "overflow-x-auto shadow-xl grow not-prose card bg-neutral text-neutral-content",
            div { class: "card-body",
                h2 { class: "card-title", "Artists" }
                table { class: "table",
                    thead {
                        tr { class: "text-neutral-content",
                            th { "Name" }
                            th { "Status" }
                            th { "Actions" }
                        }
                    }
                    tbody {
                        SuspenseBoundary {
                            fallback: |_context: SuspenseContext| rsx! { RowFallback {} },
                            ArtistsTableBody {}
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ArtistsTableBody() -> Element {
    let session = use_user()?;
    let is_owner = session()
        .ok()
        .flatten()
        .is_some_and(|user| user.permissions.contains("label_owner"));
    let show_deleted = use_context::<super::AdminPrefs>().show_deleted;
    let artists_resource = use_page(move || page_data(get_label_artists(show_deleted())))?;
    let artists = match artists_resource() {
        Ok(result) => result.artists,
        Err(error) => {
            tracing::error!("Error while getting artists for dashboard: {error}");
            Vec::new()
        }
    };

    rsx! {
        if artists.is_empty() {
            tr { td { colspan: "3", "No artists found." } }
        } else {
            for artist in artists {
                ArtistRow { artist }
            }
        }
        tr {
            td { colspan: "2" }
            td {
                if is_owner {
                    Link {
                        to: Route::CreateArtist { bad: None },
                        class: "btn btn-primary",
                        "Add"
                    }
                }
            }
        }
    }
}

#[component]
fn ArtistRow(artist: Artist) -> Element {
    rsx! {
        tr {
            td { "{artist.name}" }
            td {
                StatusBadge {
                    deleted_at: artist.deleted_at,
                    published_at: artist.published_at,
                }
            }
            td {
                Link {
                    to: Route::EditArtist {
                        artist_slug: artist.slug,
                        updated: None,
                        bad: None,
                    },
                    class: "btn btn-primary",
                    "Edit"
                }
            }
        }
    }
}

/// The label's pages table; the pages resource keys off the label id.
#[component]
fn PagesTable() -> Element {
    rsx! {
        div { class: "overflow-x-auto shadow-xl grow not-prose card bg-neutral text-neutral-content",
            div { class: "card-body",
                h2 { class: "card-title", "Pages" }
                table { class: "table",
                    thead {
                        tr { class: "text-neutral-content",
                            th { "Name" }
                            th { "Status" }
                            th { "Actions" }
                        }
                    }
                    tbody {
                        SuspenseBoundary {
                            fallback: |_context: SuspenseContext| rsx! { RowFallback {} },
                            PagesTableBody {}
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn PagesTableBody() -> Element {
    let session = use_user()?;
    let is_owner = session()
        .ok()
        .flatten()
        .is_some_and(|user| user.permissions.contains("label_owner"));
    let show_deleted = use_context::<super::AdminPrefs>().show_deleted;

    // Dependent loads: `use_page`'s `?` waits for the label, so the pages
    // resource below can use its id.
    let label_result = use_label()?;
    let label_id = match label_result() {
        Ok(result) => result.record_label.id,
        Err(error) => {
            tracing::error!("Error while getting label for pages table: {error}");
            0
        }
    };
    let pages_resource = use_page(move || page_data(get_label_pages(label_id, show_deleted())))?;
    let pages = match pages_resource() {
        Ok(result) => result.pages,
        Err(error) => {
            tracing::error!("Error while getting pages for dashboard: {error}");
            Vec::new()
        }
    };

    rsx! {
        if pages.is_empty() {
            tr { td { colspan: "3", "No pages found." } }
        } else {
            for page in pages {
                PageRow { page }
            }
        }
        tr {
            td { colspan: "2" }
            td {
                if is_owner {
                    Link {
                        to: Route::CreatePage { bad: None },
                        class: "btn btn-primary",
                        "Add"
                    }
                }
            }
        }
    }
}

#[component]
fn PageRow(page: Page) -> Element {
    rsx! {
        tr {
            td { "{page.name}" }
            td {
                StatusBadge {
                    deleted_at: page.deleted_at,
                    published_at: page.published_at,
                }
            }
            td {
                Link {
                    to: Route::EditPage {
                        slug: page.slug,
                        updated: None,
                        bad: None,
                    },
                    class: "btn btn-primary",
                    "Edit"
                }
            }
        }
    }
}

/// Three skeleton rows while a table loads (shared by both tables).
#[component]
fn RowFallback() -> Element {
    rsx! {
        for _ in 0..3 {
            tr {
                td { class: "w-full h-4 skeleton" }
                td { class: "w-full h-4 skeleton" }
                td { class: "w-full h-4 skeleton" }
            }
        }
    }
}

/// A promotion tip, rotated through the list daily.
#[component]
fn SocialMediaTips() -> Element {
    let tip = random_tip();
    rsx! {
        div { class: "overflow-x-auto shadow-xl basis-sm not-prose card bg-neutral text-neutral-content bg-base-100",
            div { class: "card-body",
                h2 { class: "card-title", "Social Tips!" }
                p { "Tips for promotion and social engagement…" }
                p { "{tip}" }
                div { class: "justify-end card-actions" }
            }
        }
    }
}

/// The promotion tip list; the daily rotation happens in [`random_tip`].
fn random_tip() -> &'static str {
    let tips: Vec<&'static str> = vec![
        "Check out our new release... <INSERT RELEASE HERE> Out on Friday!",
        "Post regularly to keep your audience engaged.",
        "Use high-quality images and videos to attract attention.",
        "Engage with your followers by responding to comments and messages.",
        "Utilize hashtags to increase the visibility of your posts.",
        "Analyze your social media metrics to understand what works best.",
        "Collaborate with influencers to reach a wider audience.",
        "Post on threads to engage with your community more effectively.",
        "Create shareable content to encourage reposts and mentions.",
        "Use trending hashtags to boost post engagement during popular times.",
        "Engage in relevant discussions and participate in group chats to build relationships.",
        "Schedule posts in advance to manage your posting timeline efficiently.",
        "Monitor the performance of your posts with tools that track reach, engagement, and conversions.",
        "Use visually appealing graphics to enhance the aesthetics of your social media content.",
        "Engage with other influencers in your niche for potential collaborations or cross-promotions.",
        "Utilize stories to add variety to your regular posts and keep followers interested.",
        "Create contests or giveaways to increase user engagement and attract new followers.",
        "Share user-generated content to show appreciation and encourage more contributions.",
        "Monitor trends in your industry and adapt your social media strategy accordingly.",
        "Use polls and quizzes as a fun way to interact with your audience and gather feedback.",
        "Get hyped for <ARTIST>'s official visualizer of '<RELEASE>' – it's coming Friday!",
        "What track are you most excited about? Let us know now by commenting on this post about <RELEASE>.",
        "Get hyped for <ARTIST>'s official visualizer of '<RELEASE>' – it's coming Friday!",
        "What track are you most excited about? Let us know now by commenting on this post about <RELEASE>.",
        "<ARTIST>'s new album, '<RELEASE>', featuring singles like '<TRACK_NAME>' and release date is <RELEASE_DATE>, drops soon! Who else can't wait?",
        "Countdown Alert: Only 2 days to listen to the debut of our <RELEASE> series. You'll be hearing from <ARTIST> every other day.",
        "Drop a ❤️ for each track you'll explore in the upcoming <RELEASE> visualizer countdown – it's art we're sharing soon!",
        "<TRACK_NAME>, the lead single off '<RELEASE>', is already shaping up to be viral! Be sure not to miss its debut and engagement prompt on release day.",
        "Our exclusive interview with <ARTIST> about '<RELEASE>' coming soon? Comment now telling us your music question for a hint or reveal below!",
        "Exclusive Tease: A leaked clip of the main visualizer countdown from <RELEASE> is out. Check it via comments to share this moment, and tag friends who'll enjoy more engagement.",
        "<RELEASE_DATE> marks our biggest launch yet! We're announcing '<TRACK_NAME>' now on social – we'd love to hear what you think!",
        "Artist Spotlight Alert: Behind the scenes from <ARTIST>, we've revealed 'TrackName' is one of many hits in the upcoming release. Vote down below which song you'll share most!",
        "The world's getting ready for '<RELEASE>' on November 15th (example date). Are you listening? Our tips will help boost your engagement levels.",
        "<ARTIST> drops a new lyric video teaser today from us – repost or save it to stay in the loop and promote even more.",
        "Coming Soon: Countdown! We've seen 'TrackName' previewed by fans, and I know your <RELEASE> date is just days away. Let's share that with others for high engagement!",
        "The visualizer countdown of '<RELEASE>' from us? It might have some special filter effects you'll see only on release day – comment what effect excites most.",
        "New Music Update: An early audio sample of one song in <RELEASE> leaked. Comment away telling us how engaged this keeps your audience! Don't repost if not sure, wait for the official announcement.",
        "<ARTIST>'s main music video teaser released today via our platform – what's your guess about the theme or songs included? Vote now!",
        "We're Officially Spilling Beans On: The release date is <RELEASE_DATE> for '<RELEASE>', and one of my favorite tracks is called '<TRACK_NAME>'. Who knew we'd be driving engagement like this all month?",
        "This upcoming <RELEASE>, including singles, dates, etc., needs your community participation. How? Share with everyone using our interactive posts – reply below!",
    ];

    // Deterministic so the SSR render and the client's hydrating render
    // pick the same tip; rotates through the list daily.
    let index = usize::try_from(chrono::Utc::now().ordinal()).unwrap_or(0) % tips.len();
    tips[index]
}

/// The days/hours/minutes/seconds between `now` and `target` (a past
/// target yields negative values, which daisyUI renders as zeros).
fn countdown_parts(now: DateTime<Utc>, target: DateTime<Utc>) -> (i64, i64, i64, i64) {
    let diff = target - now;
    (
        diff.num_days(),
        diff.num_hours() % 24,
        diff.num_minutes() % 60,
        diff.num_seconds() % 60,
    )
}

/// The upcoming-release countdown card; renders nothing when the label has
/// no future-scheduled release.
#[component]
fn UpcomingRelease() -> Element {
    rsx! {
        SuspenseBoundary {
            fallback: |_context: SuspenseContext| rsx! { Loading {} },
            UpcomingReleaseBody {}
        }
    }
}

#[component]
fn UpcomingReleaseBody() -> Element {
    let upcoming = use_page(move || page_data(get_next_scheduled_release()))?;
    let release = match upcoming() {
        Ok(Some(result)) => result.release,
        Ok(None) => return rsx! {},
        Err(error) => {
            tracing::error!("Error while getting upcoming release: {error}");
            // The error arm falls through and renders the card with a
            // default (empty) release and a zero countdown.
            Release::default()
        }
    };

    let now = Utc::now();
    let (days, hours, minutes, seconds) = countdown_parts(now, release.release_date.unwrap_or(now));

    rsx! {
        div { class: "shadow-xl grow not-prose card bg-neutral text-neutral-content bg-base-100",
            div { class: "card-body",
                h2 { class: "card-title",
                    "Upcoming Release — {release.name}"
                }
                div { class: "grid grid-flow-col auto-cols-max gap-5 pt-6 text-center justify-center-safe",
                    div { class: "flex flex-col",
                        span { class: "font-mono text-5xl countdown",
                            span { style: format!("--value:{days}") }
                        }
                        "days"
                    }
                    div { class: "flex flex-col",
                        span { class: "font-mono text-5xl countdown",
                            span { style: format!("--value:{hours}") }
                        }
                        "hours"
                    }
                    div { class: "flex flex-col",
                        span { class: "font-mono text-5xl countdown",
                            span { style: format!("--value:{minutes}") }
                        }
                        "min"
                    }
                    div { class: "flex flex-col",
                        span { class: "font-mono text-5xl countdown",
                            span { style: format!("--value:{seconds}") }
                        }
                        "sec"
                    }
                }
            }
        }
    }
}

/// The mailing-list stub card, static markup with no data behind it yet
/// (it shows a hard-coded `0`).
#[component]
fn MailingListCard() -> Element {
    rsx! {
        div { class: "shadow-xl grow not-prose card bg-neutral text-neutral-content",
            div { class: "card-body",
                h2 { class: "card-title", "Mailing list" }
                div { class: "shadow stats",
                    div { class: "stat",
                        div { class: "stat-figure text-primary",
                            svg {
                                xmlns: "http://www.w3.org/2000/svg",
                                fill: "none",
                                view_box: "0 0 24 24",
                                class: "inline-block w-8 h-8 stroke-current",
                                path {
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    stroke_width: "2",
                                    d: "M5 8h14M5 8a2 2 0 110-4h14a2 2 0 110 4M5 8v10a2 2 0 002 2h10a2 2 0 002-2V8m-9 4h4",
                                }
                            }
                        }
                        div { class: "stat-title text-neutral-content", "Email:" }
                        div { class: "stat-value text-primary", "0" }
                        div { class: "stat-desc", "subscribers" }
                    }
                }
            }
        }
    }
}

/// Housekeeping: the two-step purge of rows past their soft-delete window
/// ([`PURGE_AFTER_DAYS`] days; rules in [`crate::services::purge`]). The
/// preview lists every row it would remove by name (skipped artists
/// included), not just a count: the names ride the reactive twin's answer,
/// or — javascript off — the card's own [`get_purge_preview`] load after
/// the preview redirect.
///
/// With javascript off both steps are plain posts to `/admin/forms/purge`:
/// the preview form (empty `confirm`) 303s back to `/admin?ready=<count>`,
/// which re-renders with the listing and the confirm form (it sends
/// `confirm=1`) visible, and its redirect carries
/// `/admin?purged=<count>&skipped=<count>`. A client build drives the same
/// two steps through the `/api/purge_deleted` twin (a webview window cannot
/// follow a 303, so the no-JS query codes never reach one).
#[component]
fn PurgeCard(
    /// Rows a preview found, as a numeric string (form redirect).
    ready: Option<String>,
    /// Rows a purge removed, as a numeric string (form redirect).
    purged: Option<String>,
    /// Past-window artists a ran purge left in place (form redirect).
    skipped: Option<String>,
    /// Fixed code from a refused purge (form redirect).
    bad: Option<String>,
) -> Element {
    rsx! {
        SuspenseBoundary {
            fallback: |_context: SuspenseContext| rsx! { Loading {} },
            PurgeCardBody { ready, purged, skipped, bad }
        }
    }
}

#[component]
fn PurgeCardBody(
    ready: Option<String>,
    purged: Option<String>,
    skipped: Option<String>,
    bad: Option<String>,
) -> Element {
    let mut purge = use_action(move |confirm: bool| async move { purge_deleted(confirm).await });
    // Which step the last reactive call was: both answer `Saved(PurgeResult)`,
    // so the card has to know whether to render the answer as a preview or
    // as the done notice.
    let mut executed = use_signal(|| false);

    // Numbers that arrived in query strings are re-parsed here, so nothing
    // unvalidated is ever echoed back into the page.
    let ready_count = ready.as_deref().and_then(|code| code.parse::<usize>().ok());
    let purged_count = purged
        .as_deref()
        .and_then(|code| code.parse::<usize>().ok());
    let skipped_count = skipped
        .as_deref()
        .and_then(|code| code.parse::<usize>().ok())
        .unwrap_or(0);

    // The `?ready=` redirect leaves the listing itself to the page: this
    // GET load runs the read-only preview service (in-process during SSR,
    // over HTTP in client builds). It short-circuits when no preview is
    // pending, and the reactive path reads names off the twin's answer
    // instead.
    let wants_listing = ready.is_some();
    let listed = use_page(move || page_data(get_purge_preview(wants_listing)))?;

    // The two paths (twin answer, query codes) fold into three slots: a
    // refusal, a done notice, and a preview — the preview is what makes the
    // confirm button appear.
    let problem = refused_code(purge.value())
        .as_deref()
        .map(purge_message)
        .or_else(|| bad.as_deref().map(purge_message))
        .map(str::to_string)
        .or_else(|| transport_message(purge.value()));
    let saved = saved_row(purge.value());
    let done = saved.clone().map_or_else(
        || {
            purged_count.map(|count| {
                format!(
                    "Permanently deleted {count} items.{}",
                    skip_note(skipped_count)
                )
            })
        },
        |result| {
            executed().then(|| {
                format!(
                    "Permanently deleted {} tracks, {} releases, {} artists and {} pages.{}",
                    result.tracks.len(),
                    result.releases.len(),
                    result.artists.len(),
                    result.pages.len(),
                    skip_note(result.skipped_artists.len()),
                )
            })
        },
    );
    let preview_count = saved.as_ref().map_or_else(
        || {
            if done.is_some() { None } else { ready_count }
        },
        |result| (!executed()).then(|| result.total()),
    );
    // The listing's names: from this click's answer when the reactive path
    // just previewed, otherwise from the GET load the form redirect asked
    // for.
    let preview_rows = match saved {
        Some(result) => (!executed()).then_some(result),
        None if ready_count.is_some() => match listed() {
            Ok(rows) => Some(rows),
            Err(error) => {
                tracing::error!("Error while getting purge preview for dashboard: {error}");
                None
            }
        },
        None => None,
    };

    rsx! {
        div { class: "mt-6 not-prose card bg-base-100 shadow-xl",
            div { class: "card-body",
                h2 { class: "card-title", "Housekeeping" }
                p {
                    "Items deleted more than {PURGE_AFTER_DAYS} days ago can be removed for \
                     good. This cannot be undone."
                }
                if let Some(message) = problem {
                    InlineError { message }
                }
                if let Some(message) = done {
                    Success { message, show: true }
                }
                if let Some(count) = preview_count {
                    if count == 0 {
                        p { "Nothing has passed the {PURGE_AFTER_DAYS} day window yet." }
                    } else {
                        p {
                            "{count} items are past the window and ready to go for good. \
                             Deleted artists that a remaining row still names as its primary \
                             artist will be left in place."
                        }
                    }
                }
                if let Some(rows) = preview_rows
                    && (rows.total() > 0 || !rows.skipped_artists.is_empty())
                {
                    div { class: "flex flex-row flex-wrap gap-x-8 gap-y-2 text-sm",
                        {purge_listing("Artists", &rows.artists)}
                        {purge_listing("Releases", &rows.releases)}
                        {purge_listing("Tracks", &rows.tracks)}
                        {purge_listing("Pages", &rows.pages)}
                        {purge_listing("Artists left in place", &rows.skipped_artists)}
                    }
                }
                div { class: "card-actions flex-wrap",
                    form {
                        id: "purge-preview",
                        method: "post",
                        action: "/admin/forms/purge",
                        onsubmit: move |evt| {
                            evt.prevent_default();
                            executed.set(false);
                            purge.call(false);
                        },
                        input { r#type: "hidden", name: "confirm", value: "" }
                        button { class: "btn btn-secondary",
                            if purge.pending() && !executed() {
                                "Previewing\u{2026}"
                            } else {
                                "Preview the purge"
                            }
                        }
                    }
                    if let Some(count) = preview_count
                        && count > 0
                    {
                        form {
                            id: "purge-confirm",
                            method: "post",
                            action: "/admin/forms/purge",
                            onsubmit: move |evt| {
                                evt.prevent_default();
                                executed.set(true);
                                purge.call(true);
                            },
                            input { r#type: "hidden", name: "confirm", value: "1" }
                            button { class: "btn btn-error",
                                if purge.pending() && executed() {
                                    "Deleting\u{2026}"
                                } else {
                                    "Delete {count} items for good"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Maps the purge's fixed codes (both the `?bad=` redirect and the twin's
/// `Failed`) to card text; unknown codes fold into the retry message so a
/// hand-crafted query string can only ever pick from this table.
fn purge_message(code: &str) -> &'static str {
    match code {
        "forbidden" => "Only full administrators can run the purge.",
        _ => "The purge did not run; try again later.",
    }
}

/// One table's column of names in the preview listing; a table with
/// nothing in it adds nothing to the card.
fn purge_listing(title: &'static str, names: &[String]) -> Element {
    if names.is_empty() {
        return rsx! {};
    }
    rsx! {
        div { class: "flex flex-col",
            h3 { class: "font-semibold", "{title}" }
            ul {
                for name in names {
                    li { "{name}" }
                }
            }
        }
    }
}

/// The shared tail: past-window artists the purge was not allowed to free.
fn skip_note(skipped: usize) -> String {
    if skipped == 0 {
        String::new()
    } else {
        format!(
            " Left {skipped} deleted artists that a remaining row still names as primary artist."
        )
    }
}

// ---------------------------------------------------------------------------
// Reactive twin of the `/admin/forms/purge` handler
// ---------------------------------------------------------------------------

/// Preview (`confirm = false`) or run (`confirm = true`) the purge, answering
/// with the per-table row names (the preview lists them on the card).
///
/// Same service and same fixed codes as the javascript-off handler in
/// `crate::server::forms`; only the delivery differs.
///
/// # Errors
/// 401 when the session is gone. Refusals (permissions included) arrive as
/// `Failed` data; only a purge that could not run at all is a transport error.
#[post("/api/purge_deleted",
    db: crate::server::current_user::Db,
    user: crate::server::current_user::CurrentUser
)]
pub async fn purge_deleted(
    confirm: bool,
) -> core::result::Result<EditOutcome<PurgeResult>, HttpError> {
    let Some(session_user) = user.0.as_ref() else {
        // Guests never see the card; a hand-crafted POST gets the steer the
        // 303 handler would have given.
        return HttpError::unauthorized("Please log in to run the purge.");
    };
    match services::purge::purge_deleted_service(&db.0, Some(session_user), confirm).await {
        Ok(result) => Ok(EditOutcome::Saved(result)),
        Err(error) => {
            tracing::warn!("purge twin rejected: {error}");
            Ok(EditOutcome::Failed {
                code: purge_code(&error).to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::countdown_parts;

    fn utc(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
            .single()
            .expect("valid utc timestamp")
    }

    #[test]
    fn countdown_breaks_a_difference_into_units() {
        // 3 days, 4 hours, 5 minutes, 6 seconds out.
        let now = utc(2026, 1, 1, 0, 0, 0);
        let target = utc(2026, 1, 4, 4, 5, 6);
        assert_eq!(countdown_parts(now, target), (3, 4, 5, 6));
    }

    #[test]
    fn countdown_zero_when_target_is_now() {
        let now = utc(2026, 1, 1, 12, 0, 0);
        assert_eq!(countdown_parts(now, now), (0, 0, 0, 0));
    }

    #[test]
    fn countdown_of_a_past_target_is_negative_like_daisys_input() {
        // Durations truncate toward zero, so a past date yields
        // non-positive values; daisyUI clamps the CSS
        // `--value` to 0 when rendering.
        let now = utc(2026, 1, 5, 0, 0, 0);
        let past = utc(2026, 1, 4, 4, 5, 6);
        let (days, hours, minutes, seconds) = countdown_parts(now, past);
        assert!(days <= 0, "days {days}");
        assert!(hours <= 0 && minutes <= 0 && seconds <= 0);
    }
}
