//! The admin dashboard (MIGRATION.md Phase 4 slice 4), ported from the
//! Leptos `components::admin::dashboard` module and its `dash` pieces.
//!
//! Same card row as the original — label summary, artists table, pages
//! table, social tips, upcoming-release countdown, mailing-list stub —
//! with each card behind its own `SuspenseBoundary` (the Leptos per-card
//! `Transition`). Deviations:
//!
//! - The countdown is a snapshot of `Utc::now()` at render (SSR only, so no
//!   `set_interval` tick yet; the values land when the client builds come).
//! - Social tips rotate daily (deterministic across SSR and client
//!   renders); the Leptos original picked one per request with `rand`.

use chrono::{DateTime, Datelike, Utc};
use dioxus::prelude::*;

use crate::models::artist::Artist;
use crate::models::page::Page;
use crate::models::record_label::RecordLabel;
use crate::models::release::Release;
use crate::ui::data::{get_label_artists, get_label_pages, get_next_scheduled_release};
use crate::ui::page_data;
use crate::ui::use_label;
use crate::ui::use_page;
use crate::ui::use_user;
use crate::ui::utils::{Loading, StatusBadge};
use crate::utils::shorten_string::shorten_string;

/// The dashboard: the card row the Leptos `Dashboard` rendered.
#[component]
pub fn Dashboard() -> Element {
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
    }
}

/// Quick label info for `label_owner` sessions (the Leptos
/// `dash::record_label::RecordLabel` card; nothing for other users).
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
    // A failed label load renders the empty defaults, like the Leptos
    // store starting at its default.
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
                    a { href: "/admin/label", class: "btn btn-primary", "Edit" }
                }
            }
        }
    }
}

/// The label's artists table (the Leptos `dash::artists::ArtistsTable`).
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
    let artists_resource = use_page(move || page_data(get_label_artists()))?;
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
                    a { href: "/admin/artist", class: "btn btn-primary", "Add" }
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
                a {
                    href: format!("/admin/artist/{}", artist.slug),
                    class: "btn btn-primary",
                    "Edit"
                }
            }
        }
    }
}

/// The label's pages table (the Leptos `dash::pages::PagesTable`); the
/// pages resource keys off the label id, like the original keyed off the
/// store's label.
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
    let pages_resource = use_page(move || page_data(get_label_pages(label_id)))?;
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
                    a { href: "/admin/page", class: "btn btn-primary", "Add" }
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
                a {
                    href: format!("/admin/page/{}", page.slug),
                    class: "btn btn-primary",
                    "Edit"
                }
            }
        }
    }
}

/// Three skeleton rows while a table loads (the Leptos row fallbacks, which
/// were identical for both tables).
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

/// A static promotion tip, rotated through the list daily (the Leptos
/// `SocialMediaTips` list, kept verbatim).
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

/// The Leptos tip list; the index rotates daily so SSR and client renders
/// agree (Leptos picked randomly per request).
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
    // pick the same tip (the Leptos original shuffled per request, which
    // would mismatch once a client build exists); rotates through the list
    // daily.
    let index = usize::try_from(chrono::Utc::now().ordinal()).unwrap_or(0) % tips.len();
    tips[index]
}

/// The days/hours/minutes/seconds between `now` and `target`, using the
/// Leptos countdown's arithmetic (a past target yields negative values,
/// which daisyUI renders as zeros).
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
/// no future-scheduled release (the Leptos `UpcomingRelease`).
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
            // The Leptos Err arm fell through and rendered the card with a
            // default (empty) release and a zero countdown; kept as-is.
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
/// (the Leptos card rendered a hard-coded `0`).
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
        // The Leptos arithmetic yields non-positive values for a past date
        // (durations truncate toward zero); daisyUI clamps the CSS
        // `--value` to 0 when rendering.
        let now = utc(2026, 1, 5, 0, 0, 0);
        let past = utc(2026, 1, 4, 4, 5, 6);
        let (days, hours, minutes, seconds) = countdown_parts(now, past);
        assert!(days <= 0, "days {days}");
        assert!(hours <= 0 && minutes <= 0 && seconds <= 0);
    }
}
