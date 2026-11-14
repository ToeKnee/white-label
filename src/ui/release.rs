//! The public release page.
//!
//! Album details plus the track list, each track showing its featured artists
//! and, when it has an audio file, a `⏵ Play` button: that seeds the global
//! player bar at the bottom of the page (and the release's tracks with audio
//! as its queue), which keeps the sound going across navigation. While the
//! bar plays, the rows offer `+ Add to Queue` instead (see
//! [`crate::ui::player`]). A guest — this page has no signed-in session —
//! gets each track up to the label's play allowance (kept on the label,
//! five plays per track per browser by default): a track the browser has
//! spent shows a disabled `No plays left` in place of both buttons, and a
//! row with plays left says how many once the browser has spent any. The
//! page hands the rule and the label's number to the player whenever its
//! session answer or its release changes (see `guest_limits_apply`).
//! All the media lives in the bar: without the wasm client the page shows the
//! buttons but no players at all.
//!
//! Unlike the other public pages, a failed fetch here *redirects*: an
//! unknown artist falls back to the artists index, an unknown release to
//! that artist's page. The
//! navigation is issued with `use_navigator().replace` while the body
//! still renders `Loading`; the router re-renders the new route (SSR
//! converges on it, verified by the tests in `crate::server`).

use dioxus::prelude::*;

use crate::models::auth::User;
use crate::models::record_label::RecordLabel;
use crate::models::track_with_artists::TrackWithArtists;
use crate::ui::Route;
use crate::ui::SessionHint;
use crate::ui::assets::{display_image_url, served_file_url, track_art_url};
use crate::ui::data::{get_artist, get_release};
use crate::ui::markdown_html;
use crate::ui::page_data;
use crate::ui::player::{PlayableTrack, use_player};
use crate::ui::use_page;
use crate::ui::use_session_hint;
use crate::ui::use_user;
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

    // The play allowance's input, read once the release resolved so the
    // effect can carry the label's number: the page's own session load plus
    // any answer a reactive sign-in or sign-out just gave the chrome (the
    // navbar's `UserMenu` follows the same order for that half). Like the
    // fetches, these hooks only exist once the boundary stopped suspending,
    // and the effect still runs long before any row's click can land.
    let session_hint = use_session_hint();
    let page_session = use_user()?;
    let guest_plays = release_result.guest_plays;
    let mut rule_handler = use_player();
    use_effect(move || {
        let signed_in = page_session().ok().flatten();
        let enforce = guest_limits_apply(&session_hint(), signed_in.as_ref());
        rule_handler.write().set_listening_limits(
            enforce,
            guest_plays.unwrap_or(RecordLabel::DEFAULT_GUEST_PLAYS),
        );
    });

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
                // The album artwork rows and queue entries fall back to when a
                // track has no image of its own.
                release_image: release.primary_image.clone(),
                artist_slug: tracklist_artist_slug,
            }
        }
    }
}

/// The tracks on a release, or nothing when there are none. The tracks with
/// audio also become the queue the global player steps through when a row's
/// Play button is clicked, and each entry carries `artist_slug` so the bar
/// knows whose catalogue to draw more tracks from when that queue runs out.
/// Every row and every entry shows the track's own artwork, falling back to
/// the release's (`release_image`) and then to the site logo.
#[component]
fn TrackList(
    /// The tracks to display
    tracks: Vec<TrackWithArtists>,
    /// The release's name, used as the bar's second line when a track has no
    /// featured artists to show instead.
    release_name: String,
    /// The release's image column (stored file name, `None` when it has none):
    /// the artwork fallback for tracks without one of their own.
    release_image: Option<String>,
    /// The artist this release belongs to, by slug: every queue entry's
    /// catalogue, for the bar's auto-append.
    artist_slug: String,
) -> Element {
    if tracks.is_empty() {
        return rsx! {};
    }
    let queue = playable_queue(
        &tracks,
        &release_name,
        release_image.as_deref(),
        &artist_slug,
    );
    rsx! {
        h2 { "Tracks" }
        ul { class: "list bg-base-100",
            for track in tracks {
                Track {
                    key: "{track.track.slug}",
                    track,
                    release_image: release_image.clone(),
                    queue: queue.clone(),
                }
            }
        }
    }
}

/// One track row: artwork thumbnail (the track's own image, else this
/// release's, else the logo), name, featured artists, shortened blurb, and —
/// when the track has an audio file — a `⏵ Play` button (which hands the
/// track, with this release's audio tracks as its queue, to the global bar),
/// shown as `+ Add to Queue` while the bar plays. A guest whose browser
/// spent the label's plays of the track sees a disabled `No plays left`
/// instead of either, and a guest with plays left hears how many (the
/// player's rule, see [`crate::ui::player`]). All the buttons need the
/// client; a browser without javascript sees the buttons but no media.
#[component]
fn Track(
    /// The track to display
    track: TrackWithArtists,
    /// This release's image column: the thumbnail fallback when the track has
    /// no image of its own.
    release_image: Option<String>,
    /// This release's global-player queue (its tracks with audio), so the row
    /// can hand the bar the whole list and find its own place in it.
    queue: Vec<PlayableTrack>,
) -> Element {
    let featured = featured_artist_names(&track);
    let blurb = shorten_string(track.track.description.clone());

    let player = use_player();
    let bar_is_open = player().is_active();
    let is_loaded = player().is_playing(&track.track.slug);
    // A guest whose browser spent the label's plays of this track gets a
    // disabled `No plays left` instead of Play or Add to Queue.
    let out_of_plays = player().out_of_plays(&track.track.slug);
    // A row with plays left mentions the count once the browser has spent
    // at least one: before any spend nothing (the full allowance is not
    // worth saying), and the server — which never sees the browser's
    // counts — always sees the full allowance, so the plain page stays
    // plain.
    let plays_hint = match player().plays_left(&track.track.slug) {
        Some(left) if left > 0 && left < player().guest_allowance() => Some(plays_note(left)),
        _ => None,
    };

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
                    src: track_art_url(
                        track.track.primary_image.as_deref(),
                        release_image.as_deref(),
                    ),
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
                // to the live queue without disturbing the loaded one. A
                // guest who spent the label's plays of it gets a disabled
                // `No plays left` instead, and until then a row says how
                // many plays are left once the browser has spent one (see
                // [`crate::ui::player`]).
                if let Some(position) = queue_position {
                    if out_of_plays {
                        button {
                            class: "btn btn-ghost btn-disabled",
                            disabled: true,
                            aria_label: "No plays left for {track.track.name}",
                            "No plays left"
                        }
                    } else if bar_is_open {
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
                    if let Some(note) = plays_hint {
                        // `ml-2` keeps a little air between the button and the
                        // note: the button is inline-flex, so without it the
                        // text sits hard against the button's edge.
                        span { class: "ml-2 align-middle text-xs opacity-70", "{note}" }
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
/// each carrying what the bar shows (including the artwork fallback: the
/// track's own image, else the release's `release_image`) plus this release's
/// artist (the catalogue the bar tops up from). Featured artists (when any)
/// form the bar's second line, otherwise the release's name does. Tracks
/// without audio are left out, so previous/next step only through playable
/// tracks.
#[must_use]
fn playable_queue(
    tracks: &[TrackWithArtists],
    release_name: &str,
    release_image: Option<&str>,
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
                image_src: track_art_url(track.track.primary_image.as_deref(), release_image),
                audio_src,
                artist_slug: artist_slug.to_string(),
            })
        })
        .collect()
}

/// Whether the guest play allowance applies to this release page: the
/// reactive auth answer wins over the page's own session load (the same
/// weighing the navbar menu does), and a load failure counts as a guest
/// (the stricter side).
///
/// ```text
/// guest_limits_apply(&SessionHint::Unknown, None)          == true  // guest
/// guest_limits_apply(&SessionHint::Unknown, Some(&user))   == false // signed in
/// guest_limits_apply(&SessionHint::SignedOut, Some(&user)) == true  // just logged out
/// ```
fn guest_limits_apply(hint: &SessionHint, page_session: Option<&User>) -> bool {
    match hint {
        SessionHint::SignedIn(_) => false,
        SessionHint::SignedOut => true,
        // Nothing reactive has happened: the session this page loaded (or,
        // in a client build with no page load to carry it, the one this
        // component fetched) decides.
        SessionHint::Unknown => !page_session.is_some_and(User::is_authenticated),
    }
}

/// A guest row's note about the plays a track has left: singular for one,
/// plain for the rest. Rows only show one once the browser has spent at
/// least one play but still has some; a row out of plays carries a
/// disabled `No plays left` button instead (see [`crate::ui::player`]).
///
/// ```text
/// plays_note(1) == "1 play left"
/// plays_note(4) == "4 plays left"
/// ```
fn plays_note(left: usize) -> String {
    if left == 1 {
        "1 play left".to_string()
    } else {
        format!("{left} plays left")
    }
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
    use super::{
        audio_source, featured_artist_names, guest_limits_apply, playable_queue, plays_note,
    };
    use crate::models::artist::Artist;
    use crate::models::auth::User;
    use crate::models::track::Track;
    use crate::models::track_with_artists::TrackWithArtists;
    use crate::ui::SessionHint;

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
    fn guests_get_the_play_allowance_rule_and_members_do_not() {
        // A page with no session answer: the visitor is a guest, so the bar
        // enforces the label's play allowance.
        assert!(guest_limits_apply(&SessionHint::Unknown, None));

        // The page loaded a signed-in session: rules off, replay freely.
        // (Guest `User::default` carries the `-1` sentinel, so `id: 1` is a
        // real account.)
        let member = User {
            id: 1,
            ..User::default()
        };
        assert!(
            !guest_limits_apply(&SessionHint::Unknown, Some(&member)),
            "a signed-in page session turns the rule off"
        );
    }

    #[test]
    fn the_reactive_auth_answer_overrides_the_page_session() {
        // The same pair the navbar menu weighs: a sign-in that never
        // reloaded this page still turns the rule off, and a sign-out turns
        // it back on (the desktop case where this page's own load is stale).
        let member = User {
            id: 1,
            ..User::default()
        };
        assert!(
            !guest_limits_apply(&SessionHint::SignedIn(Box::new(member.clone())), None),
            "the just-signed-in answer wins over a stale guest load"
        );
        assert!(
            guest_limits_apply(&SessionHint::SignedOut, Some(&member)),
            "the just-signed-out answer wins over a stale signed-in load"
        );
    }

    #[test]
    fn a_failed_session_load_keeps_the_rule_on() {
        // `use_user` answers `None` both for guests and for load failures:
        // the stricter side is the safe shape for the rule.
        assert!(
            guest_limits_apply(&SessionHint::Unknown, None),
            "a missing or failed session leaves the listener treated as a guest"
        );
    }

    #[test]
    fn the_plays_note_reads_naturally() {
        assert_eq!(plays_note(1), "1 play left", "one reads as singular");
        assert_eq!(plays_note(2), "2 plays left");
        assert_eq!(plays_note(4), "4 plays left");
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

        let queue = playable_queue(
            &[first, silent, with_audio],
            "Test Release",
            None,
            "test-artist",
        );
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
        let queue = playable_queue(&[with_audio], "Test Release", None, "jimi");
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
            None,
            "test-artist",
        );
        assert_eq!(queue[0].line, "Featuring Artist 2, Artist 3");
        assert_eq!(queue[1].line, "Test Release", "no features, so the release");
    }

    #[test]
    fn a_release_without_any_audio_has_an_empty_queue() {
        let tracks = vec![track_with_artists(1, &[1]), track_with_artists(1, &[1, 2])];
        assert!(
            playable_queue(&tracks, "Test Release", None, "test-artist").is_empty(),
            "nothing playable, so nothing to queue"
        );
    }

    #[test]
    fn queue_entries_carry_the_track_art_then_the_album_art() {
        // The bar shows artwork for every entry: the track's own image when it
        // has one, otherwise the release image handed down from the page.
        let mut own_art = track_with_artists(1, &[1]);
        own_art.track.audio = Some("1700000000-one.wav".to_string());
        own_art.track.primary_image = Some("1700000000-one.png".to_string());
        let mut album_art = track_with_artists(1, &[1]);
        album_art.track.audio = Some("1700000000-two.wav".to_string());

        let queue = playable_queue(
            &[own_art, album_art],
            "Test Release",
            Some("1700000000-album.jpg"),
            "test-artist",
        );
        assert_eq!(queue[0].image_src, "/uploads/tracks/1700000000-one.png");
        assert_eq!(queue[1].image_src, "/uploads/releases/1700000000-album.jpg");

        let mut bare_track = track_with_artists(1, &[1]);
        bare_track.track.audio = Some("1700000000-two.wav".to_string());
        let queue = playable_queue(&[bare_track], "Test Release", None, "test-artist");
        assert_eq!(queue[0].image_src, "/Logo.svg", "neither, so the logo");
    }
}
