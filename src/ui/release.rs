//! The public release page.
//!
//! Album details plus the track list, each track showing its featured artists
//! and, when it has an audio file, a `⏵ Play` button: that seeds the global
//! player bar at the bottom of the page (and the release's tracks with audio
//! as its queue), which keeps the sound going across navigation. While the
//! bar plays, the rows offer `+ Add to Queue` instead (see
//! [`crate::ui::player`]). All the media lives in the bar: without the wasm
//! client the page shows the buttons but no players at all.
//!
//! Unlike the other public pages, a failed fetch here *redirects*: an
//! unknown artist falls back to the artists index, an unknown release to
//! that artist's page. The
//! navigation is issued with `use_navigator().replace` while the body
//! still renders `Loading`; the router re-renders the new route (SSR
//! converges on it, verified by the tests in `crate::server`).

use dioxus::prelude::*;

use crate::models::track_with_artists::TrackWithArtists;
use crate::ui::Route;
use crate::ui::assets::{display_image_url, served_file_url};
use crate::ui::data::{get_artist, get_release};
use crate::ui::markdown_html;
use crate::ui::page_data;
use crate::ui::player::{PlayableTrack, use_player};
use crate::ui::use_page;
use crate::ui::utils::Loading;
use crate::utils::shorten_string::shorten_string;

/// One release: cover, description and track list.
#[component]
pub fn ReleasePage(
    /// The artist slug from the URL.
    artist_slug: String,
    /// The release slug from the URL.
    release_slug: String,
) -> Element {
    rsx! {
        SuspenseBoundary {
            fallback: |_context: SuspenseContext| rsx! { Loading {} },
            // Keyed so switching releases remounts the body; the key only
            // counts on fragment children, hence the one-item loop (dioxus
            // ignores `key:` on static children — see use_page).
            for (artist_slug, release_slug) in [(artist_slug, release_slug)] {
                ReleaseBody {
                    key: "{artist_slug}/{release_slug}",
                    artist_slug,
                    release_slug,
                }
            }
        }
    }
}

/// The release page content: the artist load and then the release load,
/// each `?` suspending the boundary in turn (see the note in
/// `super::record_label::LabelHomeBody`); the page renders, or redirects —
/// see the module docs.
#[component]
fn ReleaseBody(artist_slug: String, release_slug: String) -> Element {
    let release_artist_slug = artist_slug.clone();
    // Cloning inside the closures: `use_page` keeps them as `FnMut`s it may
    // call again on a later refetch.
    let navigator = use_navigator();
    // The track list hands its artist slug to the global player: that is the
    // catalogue the queue is topped up from when it runs out.
    let tracklist_artist_slug = artist_slug.clone();

    let artist = use_page(move || page_data(get_artist(artist_slug.clone())))?;
    let artist = match artist() {
        Ok(result) => result.artist,
        Err(error) => {
            tracing::error!("Error while getting artist: {error}");
            navigator.replace(Route::ArtistsPage {});
            return rsx! { Loading {} };
        }
    };

    // The public release page: `false` because the "show deleted" toggle
    // is admin-only chrome; the service still lets a `label_owner` session
    // open unpublished rows for its own preview.
    let release = use_page(move || {
        page_data(get_release(
            release_artist_slug.clone(),
            release_slug.clone(),
            false,
        ))
    })?;
    let release_result = match release() {
        Ok(result) => result,
        Err(error) => {
            tracing::error!("Error while getting release: {error}");
            navigator.replace(Route::ArtistPage { slug: artist.slug });
            return rsx! { Loading {} };
        }
    };
    let release = release_result.release.clone();
    let tracks = release_result.tracks;

    rsx! {
        document::Title { "{release.name}" }
        article { class: "my-6 md:container md:mx-auto prose",
            h1 { "{release.name}" }
            div { class: "flex flex-wrap justify-between",
                div {
                    class: "w-1/2",
                    dangerous_inner_html: markdown_html(&release.description),
                }
                img {
                    src: display_image_url(&release.primary_image_url()),
                    alt: release.name.clone(),
                    class: "pl-6 w-1/2 h-auto",
                }
            }
            TrackList {
                tracks,
                release_name: release.name.clone(),
                artist_slug: tracklist_artist_slug,
            }
        }
    }
}

/// The tracks on a release, or nothing when there are none. The tracks with
/// audio also become the queue the global player steps through when a row's
/// Play button is clicked, and each entry carries `artist_slug` so the bar
/// knows whose catalogue to draw more tracks from when that queue runs out.
#[component]
fn TrackList(
    /// The tracks to display
    tracks: Vec<TrackWithArtists>,
    /// The release's name, used as the bar's second line when a track has no
    /// featured artists to show instead.
    release_name: String,
    /// The artist this release belongs to, by slug: every queue entry's
    /// catalogue, for the bar's auto-append.
    artist_slug: String,
) -> Element {
    if tracks.is_empty() {
        return rsx! {};
    }
    let queue = playable_queue(&tracks, &release_name, &artist_slug);
    rsx! {
        h2 { "Tracks" }
        ul { class: "list bg-base-100",
            for track in tracks {
                Track {
                    key: "{track.track.slug}",
                    track,
                    queue: queue.clone(),
                }
            }
        }
    }
}

/// One track row: thumbnail, name, featured artists, shortened blurb, and —
/// when the track has an audio file — a `⏵ Play` button (which hands the
/// track, with this release's audio tracks as its queue, to the global bar),
/// shown as `+ Add to Queue` while the bar plays. Both need the client; a
/// browser without javascript sees the buttons but no media.
#[component]
fn Track(
    /// The track to display
    track: TrackWithArtists,
    /// This release's global-player queue (its tracks with audio), so the row
    /// can hand the bar the whole list and find its own place in it.
    queue: Vec<PlayableTrack>,
) -> Element {
    let featured = featured_artist_names(&track);
    let blurb = shorten_string(track.track.description.clone());

    let player = use_player();
    let bar_is_open = player().is_active();
    let is_loaded = player().is_playing(&track.track.slug);

    // A row is in the queue exactly when it has audio; `None` rows get no
    // button at all.
    let queue_position = queue
        .iter()
        .position(|queued| queued.slug == track.track.slug);

    // One handle per handler. Play moves the queue (the row has no further
    // use for it); Add to Queue keeps a clone of just this row's entry.
    let mut play_handler = player;
    let mut add_handler = player;
    let own_entry = queue_position.map(|position| queue[position].clone());
    let queue_for_play = queue;
    let add_button_class = if is_loaded {
        "btn btn-ghost btn-active"
    } else {
        "btn btn-ghost"
    };

    rsx! {
        li { class: "list-row",
            div {
                img {
                    class: "not-prose size-10",
                    src: display_image_url(&track.track.primary_image_url()),
                    alt: track.track.name,
                }
            }
            div {
                div { "{track.track.name}" }
                div { class: "text-xs font-semibold uppercase opacity-60",
                    if !featured.is_empty() {
                        span { class: "text-xs font-semibold uppercase opacity-60",
                            "Featuring {featured}"
                        }
                    }
                }
                p { class: "text-xs list-col-wrap", "{blurb}" }
                // Uploaded audio lives under `/uploads`, which the server
                // serves to anyone, so a guest hears the same track a
                // signed-in visitor does. `⏵ Play` (shown whenever the bar
                // is closed) starts the track in the bar with this release's
                // audio tracks as the queue; while the bar plays the button
                // becomes `+ Add to Queue`, which appends this row's track
                // to the live queue without disturbing the loaded one.
                if let Some(position) = queue_position {
                    if bar_is_open {
                        // The loaded row keeps the button too: adding the
                        // playing track replays it later, on purpose.
                        if let Some(entry) = own_entry {
                            button {
                                class: "{add_button_class}",
                                aria_label: "Add {track.track.name} to queue",
                                onclick: move |_| {
                                    add_handler.write().enqueue(entry.clone());
                                },
                                "+ Add to Queue"
                            }
                        }
                    } else {
                        button {
                            class: "btn btn-ghost",
                            aria_label: "Play {track.track.name}",
                            onclick: move |_| {
                                play_handler
                                    .write()
                                    .play(queue_for_play.clone(), position);
                            },
                            // U+FE0E, as in the bar's transport: a text ⏵
                            // in the theme's colour, not an emoji.
                            "⏵\u{FE0E} Play"
                        }
                    }
                }
            }
        }
    }
}

/// The player's source for a track that has audio, or `None` when it has none
/// (and so gets no player). Absolute against the API origin in a desktop
/// window, where the page's own origin has no uploads (see [`crate::ui::assets`]).
fn audio_source(track: &TrackWithArtists) -> Option<String> {
    track.track.audio_url().map(|url| served_file_url(&url))
}

/// This release's global-player queue: the tracks with audio, in list order,
/// each carrying what the bar shows plus this release's artist (the catalogue
/// the bar tops up from). Featured artists (when any) form the bar's second
/// line, otherwise the release's name does. Tracks without audio are left
/// out, so previous/next step only through playable tracks.
#[must_use]
fn playable_queue(
    tracks: &[TrackWithArtists],
    release_name: &str,
    artist_slug: &str,
) -> Vec<PlayableTrack> {
    tracks
        .iter()
        .filter_map(|track| {
            let audio_src = audio_source(track)?;
            let featured = featured_artist_names(track);
            let line = if featured.is_empty() {
                release_name.to_string()
            } else {
                format!("Featuring {featured}")
            };
            Some(PlayableTrack {
                slug: track.track.slug.clone(),
                title: track.track.name.clone(),
                line,
                audio_src,
                artist_slug: artist_slug.to_string(),
            })
        })
        .collect()
}

/// Names of artists who contributed to the track, excluding the primary
/// artist (the `FeaturedTrackArtists` component's logic; extracted as a pure
/// function so the empty case and the join are testable). Shared with the
/// global player, which builds the same bar line for the tracks it fetches
/// when a queue runs out.
#[must_use]
pub(crate) fn featured_artist_names(track: &TrackWithArtists) -> String {
    track
        .artists
        .iter()
        .filter(|artist| artist.id != track.track.primary_artist_id)
        .map(|artist| artist.name.clone())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::{audio_source, featured_artist_names, playable_queue};
    use crate::models::artist::Artist;
    use crate::models::track::Track;
    use crate::models::track_with_artists::TrackWithArtists;

    fn track_with_artists(primary_id: i64, artist_ids: &[i64]) -> TrackWithArtists {
        let artists = artist_ids
            .iter()
            .map(|id| Artist {
                id: *id,
                name: format!("Artist {id}"),
                ..Artist::default()
            })
            .collect();
        TrackWithArtists {
            track: Track {
                primary_artist_id: primary_id,
                ..Track::default()
            },
            artists,
        }
    }

    #[test]
    fn featured_names_join_contributors_except_the_primary() {
        // id 1 is the primary artist; 2 and 3 are the contributors.
        let track = track_with_artists(1, &[1, 2, 3]);
        assert_eq!(featured_artist_names(&track), "Artist 2, Artist 3");
    }

    #[test]
    fn featured_names_empty_when_only_the_primary_performs() {
        let track = track_with_artists(1, &[1]);
        assert_eq!(featured_artist_names(&track), "");
    }

    #[test]
    fn featured_names_empty_without_artists() {
        let track = track_with_artists(1, &[]);
        assert_eq!(featured_artist_names(&track), "");
    }

    #[test]
    fn a_track_with_audio_gets_a_player_source() {
        let mut pair = track_with_artists(1, &[1]);
        pair.track.audio = Some("1700000000-castles.flac".to_string());
        assert_eq!(
            audio_source(&pair).as_deref(),
            Some("/uploads/tracks/1700000000-castles.flac")
        );
    }

    #[test]
    fn a_track_without_audio_gets_no_player() {
        let pair = track_with_artists(1, &[1]);
        assert_eq!(audio_source(&pair), None, "no file, no player");
    }

    #[test]
    fn the_queue_holds_the_tracks_with_audio_in_list_order() {
        let mut with_audio = track_with_artists(1, &[1]);
        with_audio.track.slug = "second".to_string();
        with_audio.track.name = "Second Track".to_string();
        with_audio.track.audio = Some("1700000000-second.wav".to_string());
        let mut first = track_with_artists(1, &[1]);
        first.track.slug = "first".to_string();
        first.track.name = "First Track".to_string();
        first.track.audio = Some("1700000000-first.wav".to_string());
        let silent = track_with_artists(1, &[1]);

        let queue = playable_queue(&[first, silent, with_audio], "Test Release", "test-artist");
        assert_eq!(
            queue
                .iter()
                .map(|track| track.slug.as_str())
                .collect::<Vec<_>>(),
            vec!["first", "second"],
            "rows without audio are left out"
        );
    }

    #[test]
    fn every_queue_entry_carries_the_release_artist() {
        // The bar tops the queue up from this slug when it runs out, so each
        // entry has to carry it.
        let mut with_audio = track_with_artists(1, &[1]);
        with_audio.track.audio = Some("1700000000-one.wav".to_string());
        let queue = playable_queue(&[with_audio], "Test Release", "jimi");
        assert_eq!(queue[0].artist_slug, "jimi");
    }

    #[test]
    fn queue_lines_use_featured_artists_then_the_release_name() {
        let mut featured_track = track_with_artists(1, &[1, 2, 3]);
        featured_track.track.audio = Some("1700000000-one.wav".to_string());
        let mut plain_track = track_with_artists(1, &[1]);
        plain_track.track.audio = Some("1700000000-two.wav".to_string());

        let queue = playable_queue(
            &[featured_track, plain_track],
            "Test Release",
            "test-artist",
        );
        assert_eq!(queue[0].line, "Featuring Artist 2, Artist 3");
        assert_eq!(queue[1].line, "Test Release", "no features, so the release");
    }

    #[test]
    fn a_release_without_any_audio_has_an_empty_queue() {
        let tracks = vec![track_with_artists(1, &[1]), track_with_artists(1, &[1, 2])];
        assert!(
            playable_queue(&tracks, "Test Release", "test-artist").is_empty(),
            "nothing playable, so nothing to queue"
        );
    }
}
