//! Play-statistics pages for the label, an artist and a release.
//!
//! Each page shows the same two views of one scope's counters (see
//! [`crate::services::stats`]): a line chart of per-day plays (skip, partial
//! and complete, one line each) and a table of the scope's tracks with their
//! totals. Four links above the chart pick the window: 7 days, 30 days, 1
//! year or all time — the window travels in the route's `range` query, so
//! switching it is a router navigation, not a document load.
//!
//! Design notes:
//!
//! - The chart is a hand-rolled SVG: one line per counter across the
//!   counted days, a marker at each day, and the day with the largest
//!   single counter scaled to the chart's full height. No chart crate is
//!   needed for three lines and a baseline, and no third-party dependency
//!   has to pass the licence check. The lines and markers take their
//!   colours from the daisyUI semantic palette through the
//!   `stroke-stats-*`/`fill-stats-*` utilities (`style/tailwind.css`).
//! - Tracks nobody has heard still appear in the table (with zeroes): the
//!   table describes the catalogue, not just what has been listened to.
//! - Every track row links to that track's editor, using the artist and
//!   release slugs the service carries along with the counters.

use dioxus::prelude::*;

use crate::models::track_play_stats::{DayTotals, PlaySort, TrackTotals};
use crate::ui::data::{get_artist, get_play_stats, get_release};
use crate::ui::utils::{InlineError, Loading, permission_allows};
use crate::ui::{Route, page_data, redirect, use_page, use_user};

use dioxus::router::Link;

/// The window codes the pages link between, with the labels shown on the
/// links ("all" is the default, carried by the absence of a range).
const RANGES: [(&str, &str); 4] = [
    ("7d", "7 days"),
    ("30d", "30 days"),
    ("1y", "1 year"),
    ("all", "All time"),
];

/// The range the page's URL implies: no `range` query means all time.
fn normalized_range(range: Option<&str>) -> &str {
    match range {
        None | Some("" | "all") => "all",
        Some(other) => other,
    }
}

/// The column the table's rows arrive in: no `sort` query means complete
/// plays first. An unknown code is left to the service, which answers 400 so
/// the page reports a load failure rather than quietly sorting differently
/// from the address in the bar.
fn normalized_sort(sort: Option<&str>) -> &str {
    match sort {
        None | Some("" | "complete") => "complete",
        Some(other) => other,
    }
}

/// The `range` a link carries: `None` for the window a page shows when its
/// URL says nothing, so the plain address stays plain.
fn range_param(code: &str) -> Option<&str> {
    if code == "all" { None } else { Some(code) }
}

/// The `sort` a link carries, same idea as [`range_param`]: the default
/// column (complete plays) needs no query of its own.
fn sort_param(code: &str) -> Option<&str> {
    if code == "complete" { None } else { Some(code) }
}

/// The direction a sorted column reads as, for `aria-sort` and the arrow
/// next to the header: the three names go up, the three counters go down.
fn sort_direction(code: &str) -> &'static str {
    if matches!(code, "track" | "artist" | "release") {
        "ascending"
    } else {
        "descending"
    }
}

/// The four window links, with the page's current one marked active. Each
/// link carries its code, label and target route together.
fn range_strip(current: &str, links: &[(&str, &'static str, Route)]) -> Element {
    rsx! {
        div { role: "tablist", class: "mb-4 tabs tabs-boxed not-prose",
            for (code, label, route) in links {
                Link {
                    key: "{code}",
                    to: route.clone(),
                    role: "tab",
                    class: if *code == current { "btn btn-sm btn-active" } else { "btn btn-sm" },
                    "{label}"
                }
            }
        }
    }
}

/// The page's four window links: each keeps the table's current column, so
/// switching the window never resets the sort. `route_for` names the page's
/// own route from a window/sort pair.
fn range_links_for(
    order: &str,
    route_for: impl Fn(Option<String>, Option<String>) -> Route,
) -> Vec<(&'static str, &'static str, Route)> {
    let sort = sort_param(order).map(str::to_string);
    RANGES
        .iter()
        .map(|(code, label)| {
            (
                *code,
                *label,
                route_for(range_param(code).map(str::to_string), sort.clone()),
            )
        })
        .collect()
}

/// The table's four column links: each keeps the page's current window, so
/// switching the column never resets the window. `route_for` is the same
/// helper [`range_links_for`] takes.
fn sort_links_for(
    window: &str,
    route_for: impl Fn(Option<String>, Option<String>) -> Route,
) -> Vec<(&'static str, &'static str, Route)> {
    let range = range_param(window).map(str::to_string);
    PlaySort::COLUMNS
        .iter()
        .map(|(code, label)| {
            (
                *code,
                *label,
                route_for(range.clone(), sort_param(code).map(str::to_string)),
            )
        })
        .collect()
}

/// One step of the chart's y scale: 1, 2 or 5 times a power of ten — the
/// smallest whose four-fold multiple reaches the peak, so the chart never
/// draws more than four gridlines.
fn tick_step(peak: i32) -> i32 {
    // i64 arithmetic: `candidate * 4` must not overflow on an honest
    // counter sum. No real day reaches i32's ceiling, but the loop must
    // not panic if some fixture does.
    let peak = i64::from(peak);
    let mut power = 1;
    loop {
        for candidate in [power, power * 2, power * 5] {
            if candidate * 4 >= peak {
                return i32::try_from(candidate).unwrap_or(i32::MAX);
            }
        }
        power *= 10;
    }
}

/// The chart's y scale: a value label for every tick (the baseline itself
/// counts as zero) and a light gridline at each tick above it. Built apart
/// from the svg in [`stats_graph`], which draws it behind the lines.
fn scale_nodes(peak: i32, y: &dyn Fn(i32) -> f64) -> Vec<Element> {
    let step = tick_step(peak);
    let mut nodes: Vec<Element> = Vec::new();
    let mut tick = 0;
    loop {
        nodes.push(rsx! {
            text {
                key: "tick-{tick}",
                x: "28",
                // Nudge the label onto the line's height (svg text anchors
                // on the baseline of the glyphs).
                y: format!("{:.1}", y(tick) + 3.0),
                class: "fill-current text-[10px] opacity-60",
                text_anchor: "end",
                "{tick}"
            }
        });
        if tick > 0 {
            nodes.push(rsx! {
                line {
                    key: "grid-{tick}",
                    x1: "32",
                    x2: "597",
                    y1: format!("{:.1}", y(tick)),
                    y2: format!("{:.1}", y(tick)),
                    class: "stroke-current opacity-15",
                    stroke_width: "1",
                }
            });
        }
        if tick + step > peak {
            break;
        }
        tick += step;
    }
    nodes
}

/// One polyline per counter across the series (skip, partial, complete,
/// in legend order), each with a marker at every counted day. Built apart
/// from the svg in [`stats_graph`] so each finished node goes straight in.
fn counter_nodes(
    series: &[DayTotals],
    x: &dyn Fn(chrono::NaiveDate) -> f64,
    y: &dyn Fn(i32) -> f64,
) -> Vec<Element> {
    /// The line class, marker class and counter of one chart line.
    type CounterLine = (&'static str, &'static str, fn(&DayTotals) -> i32);
    let counters: [CounterLine; 3] = [
        ("stroke-stats-skip", "fill-stats-skip", |day| day.skips),
        ("stroke-stats-partial", "fill-stats-partial", |day| {
            day.partial_plays
        }),
        ("stroke-stats-complete", "fill-stats-complete", |day| {
            day.complete_plays
        }),
    ];
    let mut drawn: Vec<Element> = Vec::new();
    for (line_class, marker_class, count_of) in counters {
        let points: Vec<String> = series
            .iter()
            .map(|day| format!("{:.1},{:.1}", x(day.day), y(count_of(day))))
            .collect();
        drawn.push(rsx! {
            polyline {
                key: "{line_class}",
                points: points.join(" "),
                fill: "none",
                stroke_width: "2",
                class: line_class,
            }
        });
        for (index, day) in series.iter().enumerate() {
            drawn.push(rsx! {
                circle {
                    key: "{marker_class}-{index}",
                    cx: format!("{:.1}", x(day.day)),
                    cy: format!("{:.1}", y(count_of(day))),
                    r: "2.5",
                    class: marker_class,
                }
            });
        }
    }
    drawn
}

/// The line chart: one line per counter across the counted days (skip,
/// partial, complete), a marker on every point, and a labelled y scale of
/// light gridlines (the baseline itself is zero), with the day that has
/// the largest single counter scaled to the chart's full height. Days with
/// no counted play are absent from the series, so a quiet stretch costs
/// nothing to draw and the lines simply span it; an entirely quiet range
/// gets a line of text instead.
fn stats_graph(series: &[DayTotals]) -> Element {
    if series.is_empty() {
        return rsx! {
            p { class: "not-prose text-base-content", "No counted plays in this range." }
        };
    }

    // A fold instead of an iterator chain ending in `max()`: the largest
    // of the three counters on each day, and the largest day overall.
    let peak = series.iter().fold(1, |peak, day| {
        peak.max(day.complete_plays)
            .max(day.partial_plays)
            .max(day.skips)
    });

    // Geometry: a 600-wide viewBox with the baseline at y = 150 and at
    // most 130 of drawing above it; the 20 below take the day labels and
    // the 40 at the left take the y scale's tick labels. Days map to their
    // calendar position across what is left of the width (35 to 595, so a
    // marker at either end keeps its 2.5 of radius inside the frame), so a
    // quiet week spreads its neighbours apart; a lone counted day sits in
    // the middle.
    let first = series[0].day;
    let last = series[series.len() - 1].day;
    let span_days = (last - first).num_days();
    #[expect(
        clippy::cast_precision_loss,
        reason = "day offsets into f64: counting past the exact-integer limit would take ~290 million years of days"
    )]
    let x = |day: chrono::NaiveDate| -> f64 {
        if span_days == 0 {
            315.0
        } else {
            35.0 + (day - first).num_days() as f64 * 560.0 / span_days as f64
        }
    };
    let y = |count: i32| -> f64 { 150.0 - f64::from(count) * 130.0 / f64::from(peak) };
    let first_label = series[0].day.format("%d %b").to_string();
    let last_label = series[series.len() - 1].day.format("%d %b").to_string();
    // Gridlines first, so the data lines stand in front of them.
    let scale = scale_nodes(peak, &y);
    let drawn = counter_nodes(series, &x, &y);

    rsx! {
        div { class: "overflow-x-auto not-prose",
            svg {
                xmlns: "http://www.w3.org/2000/svg",
                view_box: "0 0 600 170",
                width: "600",
                height: "170",
                // The baseline the lines hang above (and the scale's zero).
                line {
                    x1: "0",
                    y1: "150",
                    x2: "600",
                    y2: "150",
                    class: "stroke-current opacity-40",
                    stroke_width: "1",
                }
                for node in scale {
                    {node}
                }
                for node in drawn {
                    {node}
                }
                // The window's first and last counted day, so the lines
                // have ends to read against.
                text {
                    x: "32",
                    y: "166",
                    class: "fill-current text-[10px]",
                    text_anchor: "start",
                    "{first_label}"
                }
                text {
                    x: "600",
                    y: "166",
                    class: "fill-current text-[10px]",
                    text_anchor: "end",
                    "{last_label}"
                }
            }
        }
        // Which line is which, in the order they are drawn: a short
        // stroke of each colour instead of the old bar-chart swatch.
        div { class: "flex gap-6 not-prose mt-2",
            for (swatch, name) in [
                ("bg-stats-skip", "Skips"),
                ("bg-stats-partial", "Partial"),
                ("bg-stats-complete", "Complete"),
            ] {
                span { key: "{name}", class: "inline-flex items-center gap-2 text-sm",
                    span { class: "inline-block w-4 h-[3px] rounded-full {swatch}" }
                    "{name}"
                }
            }
        }
    }
}

/// The track table: every live track in the scope (zero plays included),
/// each linking to its editor, with its artist, its release and the
/// range's counters alongside. Its six headers are links, one per
/// [`PlaySort::COLUMNS`] entry (`headers` carries the current window along
/// with each column), so re-sorting is another navigation rather than a
/// page load — and a visitor without javascript still gets the table
/// sorted, by whatever its URL asks for.
fn stats_table(
    rows: &[TrackTotals],
    current: &str,
    headers: &[(&str, &'static str, Route)],
) -> Element {
    rsx! {
        div { class: "overflow-x-auto",
            table { class: "table",
                thead {
                    tr {
                        for (code, label, route) in headers {
                            // `aria-sort="none"` marks a column that can be
                            // sorted but currently is not; the arrow shows the
                            // direction without relying on colour.
                            th {
                                key: "{code}",
                                scope: "col",
                                aria_sort: if *code == current {
                                    sort_direction(code)
                                } else {
                                    "none"
                                },
                                Link {
                                    to: route.clone(),
                                    "{label}"
                                    if *code == current {
                                        // U+FE0E keeps the arrow a text glyph.
                                        span {
                                            aria_hidden: "true",
                                            if matches!(*code, "track" | "artist" | "release") {
                                                "↑"
                                            } else {
                                                "↓"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                tbody {
                    if rows.is_empty() {
                        tr { td { class: "text-base-content", "No tracks to show here." } }
                    }
                    for row in rows {
                        tr { key: "{row.track_slug}",
                            td {
                                Link {
                                    to: Route::EditTrack {
                                        artist_slug: row.artist_slug.clone(),
                                        release_slug: row.release_slug.clone(),
                                        track_slug: row.track_slug.clone(),
                                        updated: None,
                                        bad: None,
                                        uploaded: None,
                                        bad_upload: None,
                                    },
                                    "{row.track_name}"
                                }
                            }
                            td { "{row.artist_name}" }
                            td { "{row.release_name}" }
                            td { "{row.skips}" }
                            td { "{row.partial_plays}" }
                            td { "{row.complete_plays}" }
                        }
                    }
                }
            }
        }
    }
}

/// The body every stats page shares once its scope has been named: gate,
/// fetch (scoped by `scope`, `slug`, the `window` range code and the `order`
/// column code), chart and table (whose header links arrive in `headers`).
/// The `?` on the call in each page body re-raises the load's suspense
/// marker at component level (an embedded `Element` value would not bubble
/// it), so the enclosing `SuspenseBoundary` waits for the fetch like
/// everywhere else in the admin area.
fn stats_body(
    scope: &'static str,
    slug: String,
    window: String,
    order: String,
    headers: &[(&'static str, &'static str, Route)],
) -> Element {
    let session = use_user()?;
    let user = session().ok().flatten();
    if !permission_allows(user.as_ref(), "label_owner") {
        redirect(Route::Dashboard {
            ready: None,
            purged: None,
            skipped: None,
            bad: None,
            files: None,
        });
        return rsx! { Loading {} };
    }

    // The scope triple moves into the load closure (which clones it per
    // retry); the log line is built from it first. The column code is kept
    // alongside for the table's header marking.
    let load_name = format!("play statistics for the {scope} {slug}");
    let column = order.clone();
    let stats_scope = scope.to_string();
    let stats_resource = use_page(move || {
        page_data(get_play_stats(
            stats_scope.clone(),
            slug.clone(),
            window.clone(),
            Some(order.clone()),
        ))
    })?;
    let stats = match stats_resource() {
        Ok(stats) => stats,
        Err(error) => {
            tracing::error!("Error while getting {load_name}: {error}");
            return rsx! {
                InlineError { message: "Could not load the play statistics, try again later." }
            };
        }
    };

    rsx! {
        div { class: "grid gap-4",
            {stats_graph(&stats.series)}
            {stats_table(&stats.rows, &column, headers)}
        }
    }
}

/// The label's play statistics.
#[component]
pub fn LabelStats(
    /// The statistics window: `7d`, `30d`, `1y`, or nothing for all time.
    range: Option<String>,
    /// The table's column: `track`, `skips`, `partial`, or nothing for
    /// complete plays.
    sort: Option<String>,
) -> Element {
    rsx! {
        SuspenseBoundary {
            fallback: |_context: SuspenseContext| rsx! { Loading {} },
            // Keyed so switching a window or a column remounts the body; the
            // key only counts on fragment children, hence the one-item loop
            // (dioxus ignores `key:` on static children — see use_page).
            for (range, sort) in [(range, sort)] {
                LabelStatsBody {
                    key: "{normalized_range(range.as_deref())}/{normalized_sort(sort.as_deref())}",
                    range,
                    sort,
                }
            }
        }
    }
}

#[component]
fn LabelStatsBody(range: Option<String>, sort: Option<String>) -> Element {
    let window = normalized_range(range.as_deref()).to_string();
    let order = normalized_sort(sort.as_deref()).to_string();
    // The label scope addresses the single label; its slug slot is unused
    // (see [`crate::ui::data::get_play_stats`]), hence the placeholder.
    // Both knobs keep their partner: a window link carries the current
    // column, a column link the current window.
    let range_links = range_links_for(&order, |range, sort| Route::LabelStats { range, sort });
    let sort_links = sort_links_for(&window, |range, sort| Route::LabelStats { range, sort });
    // The window and column move into the fetch, so the strip is built first.
    let strip = range_strip(&window, &range_links);
    let body = stats_body("label", "-".to_string(), window, order, &sort_links)?;

    rsx! {
        document::Title { "Play statistics" }
        h1 { "Play statistics" }
        // No back link to the label editor: the admin side menu (which
        // carried the link here) already offers it on every admin page.
        {strip}
        {body}
    }
}

/// One artist's play statistics.
#[component]
pub fn ArtistStats(
    /// The artist slug from the URL.
    artist_slug: String,
    /// The statistics window: `7d`, `30d`, `1y`, or nothing for all time.
    range: Option<String>,
    /// The table's column: `track`, `skips`, `partial`, or nothing for
    /// complete plays.
    sort: Option<String>,
) -> Element {
    rsx! {
        SuspenseBoundary {
            fallback: |_context: SuspenseContext| rsx! { Loading {} },
            for (artist_slug, range, sort) in [(artist_slug, range, sort)] {
                ArtistStatsBody {
                    key: "{artist_slug}/{normalized_range(range.as_deref())}/{normalized_sort(sort.as_deref())}",
                    artist_slug,
                    range,
                    sort,
                }
            }
        }
    }
}

#[component]
fn ArtistStatsBody(artist_slug: String, range: Option<String>, sort: Option<String>) -> Element {
    // The header carries the artist's name; a slug that matches no artist
    // redirects to NotFound, as in the other artist editors.
    let artist_name_slug = artist_slug.clone();
    let artist_resource = use_page(move || page_data(get_artist(artist_name_slug.clone())))?;
    let artist_name = match artist_resource() {
        Ok(result) => result.artist.name,
        Err(error) => {
            tracing::error!(
                "Error while getting artist for the stats page (slug {artist_slug}): {error}"
            );
            redirect(Route::NotFound {
                segments: vec!["admin".into(), "artist-not-found".into()],
            });
            return rsx! { Loading {} };
        }
    };

    let window = normalized_range(range.as_deref()).to_string();
    let order = normalized_sort(sort.as_deref()).to_string();
    // Each link keeps its partner knob (the current column, respectively the
    // current window), so switching one never loses the other.
    let range_links = range_links_for(&order, |range, sort| Route::ArtistStats {
        artist_slug: artist_slug.clone(),
        range,
        sort,
    });
    let sort_links = sort_links_for(&window, |range, sort| Route::ArtistStats {
        artist_slug: artist_slug.clone(),
        range,
        sort,
    });
    // The window and column move into the fetch, so the strip is built first.
    let strip = range_strip(&window, &range_links);
    let body = stats_body("artist", artist_slug, window, order, &sort_links)?;

    rsx! {
        document::Title { "{artist_name} Play statistics" }
        h1 { "{artist_name} Play statistics" }
        {strip}
        {body}
    }
}

/// One release's play statistics.
#[component]
pub fn ReleaseStats(
    /// The artist slug from the URL.
    artist_slug: String,
    /// The release slug from the URL.
    release_slug: String,
    /// The statistics window: `7d`, `30d`, `1y`, or nothing for all time.
    range: Option<String>,
    /// The table's column: `track`, `skips`, `partial`, or nothing for
    /// complete plays.
    sort: Option<String>,
) -> Element {
    rsx! {
        SuspenseBoundary {
            fallback: |_context: SuspenseContext| rsx! { Loading {} },
            for (artist_slug, release_slug, range, sort) in [(artist_slug, release_slug, range, sort)] {
                ReleaseStatsBody {
                    key: "{artist_slug}/{release_slug}/{normalized_range(range.as_deref())}/{normalized_sort(sort.as_deref())}",
                    artist_slug,
                    release_slug,
                    range,
                    sort,
                }
            }
        }
    }
}

#[component]
fn ReleaseStatsBody(
    artist_slug: String,
    release_slug: String,
    range: Option<String>,
    sort: Option<String>,
) -> Element {
    // The header carries the release's name; a slug that matches no release
    // steers back to the tracks list, as the release editors do.
    let release_artist_slug = artist_slug.clone();
    let release_slug_for_resource = release_slug.clone();
    let release_resource = use_page(move || {
        page_data(get_release(
            release_artist_slug.clone(),
            release_slug_for_resource.clone(),
            false,
        ))
    })?;
    let release_name = match release_resource() {
        Ok(result) => result.release.name,
        Err(error) => {
            tracing::error!(
                "Error while getting release for the stats page (slug {release_slug}): {error}"
            );
            // This arm diverges, so the slugs can move into the steer.
            redirect(Route::Tracks {
                artist_slug,
                release_slug,
            });
            return rsx! { Loading {} };
        }
    };

    let window = normalized_range(range.as_deref()).to_string();
    let order = normalized_sort(sort.as_deref()).to_string();
    // Each link keeps its partner knob (the current column, respectively the
    // current window), so switching one never loses the other.
    let range_links = range_links_for(&order, |range, sort| Route::ReleaseStats {
        artist_slug: artist_slug.clone(),
        release_slug: release_slug.clone(),
        range,
        sort,
    });
    let sort_links = sort_links_for(&window, |range, sort| Route::ReleaseStats {
        artist_slug: artist_slug.clone(),
        release_slug: release_slug.clone(),
        range,
        sort,
    });
    // The strip is built before the window and slugs move into the fetch;
    // the back link below gets its own copies.
    let strip = range_strip(&window, &range_links);
    let back_route = Route::EditRelease {
        artist_slug: artist_slug.clone(),
        release_slug: release_slug.clone(),
        updated: None,
        bad: None,
        uploaded: None,
        bad_upload: None,
    };
    let body = stats_body("release", release_slug, window, order, &sort_links)?;

    rsx! {
        document::Title { "{release_name} Play statistics" }
        h1 { "{release_name} Play statistics" }
        p {
            Link {
                to: back_route,
                class: "not-prose btn btn-ghost",
                "Back to the release editor"
            }
        }
        {strip}
        {body}
    }
}

#[cfg(test)]
mod tests {
    use super::tick_step;

    #[test]
    fn the_y_scale_step_is_a_nice_number() {
        // 1, 2 or 5 times a power of ten, sized to the peak.
        assert_eq!(tick_step(1), 1);
        assert_eq!(tick_step(4), 1);
        assert_eq!(tick_step(5), 2);
        assert_eq!(tick_step(8), 2);
        assert_eq!(tick_step(250), 100);
        // ... and any peak lands on at most four gridlines (plus the
        // baseline's zero label).
        for peak in 1..=20_000 {
            let step = tick_step(peak);
            assert!(
                peak / step <= 4,
                "{peak} gets at most four gridlines, got one every {step}"
            );
        }
    }
}
