//! The global player: one track playing at a time, kept alive while the user
//! navigates.
//!
//! [`PlayerState`](crate::ui::player::PlayerState) lives in a context that
//! [`crate::ui::shell::App`] provides, so it outlives the router. Its bar,
//! [`GlobalPlayer`](crate::ui::player::GlobalPlayer), is rendered by `AppShell`
//! as page chrome rather than inside a page, which is what keeps the sound
//! going: navigating swaps the `Outlet`'s contents, and the bar's `<audio>`
//! element is never torn down while a track stays loaded. The bar also shows
//! the loaded track's artwork — its own image, else the album art of the
//! release it sits on, else the site logo (see
//! [`crate::ui::assets::track_art_url`]).
//!
//! While the bar plays
//! ([`PlayerState::is_active`](crate::ui::player::PlayerState::is_active))
//! the release page's rows offer `+ Add to Queue` instead of `⏵ Play`:
//! appending to the end never disturbs the loaded track, so a listener can
//! grow the queue from any release page.
//!
//! The Play button seeds the queue from the release's tracks with audio
//! (whichever list the click came from); `+ Add to Queue` ([`PlayerState::
//! enqueue`]) appends to the live queue. previous/next step through it and
//! stop at either end. All the media lives in the bar's `<audio>`: without
//! javascript — or against a server serving no wasm bundle — the page offers
//! the buttons but no media ever appears (the player needs the client).
//!
//! When the queue runs out — a `⏭` at the end, or a track ending there — the
//! bar asks the server for a fresh handful of the playing track's artist's
//! other audio tracks (random picks, [`crate::ui::data::
//! get_continuation_tracks`]) and keeps playing: the fetched tracks join the
//! end of the queue and the bar steps onto the first of them. A fetch that
//! comes back with nothing marks the queue
//! ([`mark_catalogue_exhausted`](crate::ui::player::PlayerState::mark_catalogue_exhausted)) so
//! the bar stops asking until the next Play.

use dioxus::prelude::*;

use crate::api_types::track::UpcomingAudio;
use crate::ui::assets::{served_file_url, track_art_url};
use crate::ui::data::get_continuation_tracks;
use crate::ui::release::featured_artist_names;

/// A track the global player can play: everything its bar needs to show and
/// load, as plain data so it can sit in a context signal and travel in a queue.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayableTrack {
    /// Identifies the track in the queue, and keys its `<audio>` element so
    /// switching tracks builds a fresh element instead of re-pointing the old
    /// one mid-playback.
    pub slug: String,
    /// The bar's main line.
    pub title: String,
    /// The bar's second line: featuring artists, or the release it came from.
    pub line: String,
    /// The loaded track's artwork: its own image, else the album art of the
    /// release it sits on, else the site logo, already made absolute where the
    /// platform needs it (see [`crate::ui::assets::track_art_url`]).
    pub image_src: String,
    /// Where the audio lives, already made absolute where the platform needs it
    /// (see [`crate::ui::assets::served_file_url`]).
    pub audio_src: String,
    /// The artist whose catalogue this track belongs to: when the queue runs
    /// out the bar asks that artist for more tracks by this slug.
    pub artist_slug: String,
}

/// What the global player has loaded, and where in the queue it stands.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PlayerState {
    /// The tracks previous/next step through.
    queue: Vec<PlayableTrack>,
    /// The loaded track's index in the queue, or `None` while nothing is loaded
    /// (no bar, and each page keeps its own players).
    position: Option<usize>,
    /// True once a continuation fetch came back with nothing for the current
    /// queue: the artist has no playable track left outside the queue, so the
    /// bar stops asking until the next [`PlayerState::play`].
    catalogue_exhausted: bool,
}

impl PlayerState {
    /// True once a track is loaded: the bar shows, and the pages' own per-track
    /// players stand down in its favour.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.position.is_some()
    }

    /// The loaded track, or `None` when nothing is playing.
    #[must_use]
    pub fn current(&self) -> Option<&PlayableTrack> {
        self.position.and_then(|index| self.queue.get(index))
    }

    /// True when `slug` is the track the bar is showing.
    #[must_use]
    pub fn is_playing(&self, slug: &str) -> bool {
        self.current().is_some_and(|track| track.slug == slug)
    }

    /// One-based place of the loaded track in the queue, and how many tracks
    /// the queue holds (`None` while nothing is loaded). The bar shows this as
    /// "2 of 9".
    #[must_use]
    pub fn place(&self) -> Option<(usize, usize)> {
        self.position.map(|index| (index + 1, self.queue.len()))
    }

    /// Load `queue` and start the track at `position`.
    ///
    /// A position the queue doesn't hold (a list that changed under the click)
    /// starts at the first track rather than playing nothing, and an empty
    /// queue leaves the player closed rather than showing an empty bar.
    pub fn play(&mut self, queue: Vec<PlayableTrack>, position: usize) {
        let start = if position < queue.len() { position } else { 0 };
        self.position = if queue.is_empty() { None } else { Some(start) };
        self.queue = queue;
        // A fresh queue gets a fresh look at the artist's catalogue, even
        // when it replays the release that ran out.
        self.catalogue_exhausted = false;
    }

    /// Step to the next track. At the end of the queue the album is finished,
    /// so it stays loaded where it is (the bar can be replayed or skipped
    /// back); `true` only when the position actually moved.
    pub const fn next_track(&mut self) -> bool {
        match self.position {
            Some(index) if index + 1 < self.queue.len() => {
                self.position = Some(index + 1);
                true
            }
            _ => false,
        }
    }

    /// Step back to the previous track, staying put at the front of the
    /// queue. `true` only when the position moved.
    pub const fn previous_track(&mut self) -> bool {
        match self.position {
            Some(index) if index > 0 => {
                self.position = Some(index - 1);
                true
            }
            _ => false,
        }
    }

    /// Put the player away: the bar goes, and the rows offer `⏵ Play`
    /// again.
    pub fn close(&mut self) {
        self.queue.clear();
        self.position = None;
        self.catalogue_exhausted = false;
    }

    /// Append `track` to the end of the live queue — what a row's
    /// `+ Add to Queue` button does while the bar plays. The loaded track's
    /// place never moves, and a closed player has nothing to extend, so it
    /// answers `false` and changes nothing.
    pub fn enqueue(&mut self, track: PlayableTrack) -> bool {
        if self.position.is_some() {
            self.queue.push(track);
            true
        } else {
            false
        }
    }

    /// The queue's slugs in queue order (duplicates included): a continuation
    /// fetch leaves out everything it lists, so topping up never re-adds a
    /// track the queue already holds.
    #[must_use]
    pub fn queued_slugs(&self) -> Vec<String> {
        self.queue.iter().map(|track| track.slug.clone()).collect()
    }

    /// Append a fetched continuation batch to the end of the live queue, the
    /// same way [`PlayerState::enqueue`] appends one row: the loaded
    /// track's place never moves. A closed player has nothing to extend (a
    /// fetch that lands after `close` just stops), and an empty batch changes
    /// nothing; `true` only when the queue grew.
    pub fn append_batch(&mut self, tracks: Vec<PlayableTrack>) -> bool {
        if self.position.is_some() && !tracks.is_empty() {
            self.queue.extend(tracks);
            true
        } else {
            false
        }
    }

    /// Note that the artist's catalogue has nothing left for this queue, so
    /// the bar stops fetching until the next [`PlayerState::play`].
    pub const fn mark_catalogue_exhausted(&mut self) {
        self.catalogue_exhausted = true;
    }

    /// True once a continuation fetch has come back with nothing for this
    /// queue.
    #[must_use]
    pub const fn is_catalogue_exhausted(&self) -> bool {
        self.catalogue_exhausted
    }
}

/// Map one fetched continuation track onto a queue entry, with the same shape
/// a release row's entry has (see [`crate::ui::release`]'s queue): the audio
/// made absolute for this platform, featuring artists or the release it came
/// from as the bar's second line, and the artwork fallback (the track's own
/// image, else the fetched release's).
#[must_use]
fn playable_from_upcoming(upcoming: &UpcomingAudio, artist_slug: &str) -> Option<PlayableTrack> {
    let audio_src = upcoming
        .track
        .track
        .audio_url()
        .map(|url| served_file_url(&url))?;
    let featured = featured_artist_names(&upcoming.track);
    let line = if featured.is_empty() {
        upcoming.release_name.clone()
    } else {
        format!("Featuring {featured}")
    };
    Some(PlayableTrack {
        slug: upcoming.track.track.slug.clone(),
        title: upcoming.track.track.name.clone(),
        line,
        image_src: track_art_url(
            upcoming.track.track.primary_image.as_deref(),
            upcoming.release_image.as_deref(),
        ),
        audio_src,
        artist_slug: artist_slug.to_string(),
    })
}

/// Fetch a continuation batch and append it to the live queue: an empty
/// answer marks the catalogue exhausted, and a batch that lands steps the bar
/// onto its first track — whichever of them asked (a `⏭` at the queue's end,
/// or a track ending there) was asking for the next track to play. A failed
/// fetch logs and changes nothing: the listener still has the queue they had,
/// and the next end-of-queue try asks again.
// `Signal` is deliberately `!Send` (it lives on the component's local
// executor), so this future can never be `Send`; `spawn` runs it there.
#[expect(
    clippy::future_not_send,
    reason = "carries a dioxus Signal, which is !Send by design; this future only ever runs on the component's executor"
)]
async fn continue_queue(
    artist_slug: String,
    exclude_slugs: String,
    mut player: Signal<PlayerState>,
) {
    match get_continuation_tracks(artist_slug.clone(), exclude_slugs).await {
        Ok(batch) if batch.tracks.is_empty() => {
            player.write().mark_catalogue_exhausted();
        }
        Ok(batch) => {
            let queue_batch: Vec<PlayableTrack> = batch
                .tracks
                .iter()
                .filter_map(|upcoming| playable_from_upcoming(upcoming, &artist_slug))
                .collect();
            if player.write().append_batch(queue_batch) {
                player.write().next_track();
            }
        }
        Err(error) => {
            tracing::warn!("could not top up the player queue: {error}");
        }
    }
}

/// The [`PlayerState`] context that [`crate::ui::shell::App`] provides.
///
/// ```ignore
/// let mut player = use_player();
/// player.play(queue, position);
/// ```
#[must_use]
pub fn use_player() -> Signal<PlayerState> {
    use_context()
}

/// The player bar: the loaded track, where it came from, the browser's own
/// transport, and previous/next/close.
///
/// It renders nothing while the player is closed, and sits sticky to the bottom
/// of the page chrome so it follows the reader down the page without covering
/// content when it isn't needed.
#[component]
pub fn GlobalPlayer() -> Element {
    let player = use_player();
    let Some(current) = player().current().cloned() else {
        return rsx! {};
    };
    let place = player().place().unwrap_or((0, 0));

    // One handle per handler: signals are handles, so each closure can move its
    // own copy and mutate the same context state.
    let mut back_handler = player;
    let mut forward_handler = player;
    let mut close_handler = player;
    let mut ended_handler = player;
    // The queue's artist, for the continuation fetches the forward button and
    // `onended` may start (a queue can mix releases, but not artists: every
    // entry carries the slug of the page it was added from).
    let forward_artist = current.artist_slug.clone();
    let ended_artist = current.artist_slug.clone();

    rsx! {
        div {
            id: "global-player",
            class: "sticky bottom-0 z-40 border-t bg-base-200 text-base-content",
            div {
                class: "container mx-auto flex flex-wrap items-center gap-2 p-2 not-prose",
                // The loaded track's artwork, resolved when the entry was
                // built (own image, else album art, else the logo), so the
                // bar shows something for every queue it is handed.
                img {
                    class: "size-10 rounded",
                    src: current.image_src.clone(),
                    alt: current.title.clone(),
                }
                div { class: "min-w-40 flex-1",
                    div { class: "font-semibold", "{current.title}" }
                    div { class: "text-xs opacity-70",
                        "{current.line} · {place.0} of {place.1}"
                    }
                }
                // Rebuilt whenever the loaded track changes (the key), which is
                // what starts the new file playing without touching a track that
                // is already under way. `onended` walks the queue; at the end of
                // it, rather than going silent, it asks the server for more of
                // the artist's tracks and steps onto the first one fetched.
                audio {
                    key: "{current.slug}",
                    class: "w-full min-w-48 max-w-md",
                    controls: true,
                    autoplay: true,
                    src: current.audio_src,
                    onended: move |_| {
                        let moved = ended_handler.write().next_track();
                        if !moved && !ended_handler.read().is_catalogue_exhausted() {
                            let exclude = ended_handler.read().queued_slugs().join(",");
                            spawn(continue_queue(ended_artist.clone(), exclude, ended_handler));
                        }
                    },
                }
                // The U+FE0E after each glyph asks for text presentation
                // (⏮⏭⏏ are emoji-first codepoints, and a colour-emoji font
                // ignores CSS colours). `player-glyph` (see
                // `style/tailwind.css`) pins text fonts carrying these
                // glyphs, because Chromium's font fallback can reach the
                // emoji font even with U+FE0E; as text they inherit the
                // bar's `text-base-content`.
                button {
                    class: "btn btn-ghost btn-sm player-glyph",
                    aria_label: "Previous track",
                    onclick: move |_| {
                        back_handler.write().previous_track();
                    },
                    "⏮\u{FE0E}"
                }
                button {
                    class: "btn btn-ghost btn-sm player-glyph",
                    aria_label: "Next track",
                    onclick: move |_| {
                        // The same end-of-queue continuation as `onended`:
                        // a `⏭` pressed at the end is asking for more music,
                        // not for the bar to sit still.
                        let moved = forward_handler.write().next_track();
                        if !moved && !forward_handler.read().is_catalogue_exhausted() {
                            let exclude = forward_handler.read().queued_slugs().join(",");
                            spawn(continue_queue(
                                forward_artist.clone(),
                                exclude,
                                forward_handler,
                            ));
                        }
                    },
                    "⏭\u{FE0E}"
                }
                button {
                    class: "btn btn-ghost btn-sm player-glyph",
                    aria_label: "Close the player",
                    onclick: move |_| {
                        close_handler.write().close();
                    },
                    "⏏\u{FE0E}"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PlayableTrack, PlayerState, playable_from_upcoming};
    use crate::api_types::track::UpcomingAudio;
    use crate::models::track::Track;
    use crate::models::track_with_artists::TrackWithArtists;

    fn queue(names: &[&str]) -> Vec<PlayableTrack> {
        names
            .iter()
            .map(|name| PlayableTrack {
                slug: format!("slug-{name}"),
                title: (*name).to_string(),
                line: "the release".to_string(),
                image_src: format!("/uploads/tracks/{name}.png"),
                audio_src: format!("/uploads/tracks/{name}.wav"),
                artist_slug: "test-artist".to_string(),
            })
            .collect()
    }

    /// One fetched continuation track, with whichever artwork the test is
    /// about; the bar only queues audio tracks, so the file is always set.
    fn upcoming(track_image: Option<&str>, release_image: Option<&str>) -> UpcomingAudio {
        UpcomingAudio {
            track: TrackWithArtists {
                track: Track {
                    slug: "upcoming-one".to_string(),
                    name: "Upcoming One".to_string(),
                    audio: Some("1700000000-one.wav".to_string()),
                    primary_image: track_image.map(str::to_string),
                    ..Track::default()
                },
                artists: Vec::new(),
            },
            release_name: "Upcoming Release".to_string(),
            release_image: release_image.map(str::to_string),
        }
    }

    #[test]
    fn fetched_tracks_get_their_own_art_then_the_album_art() {
        // The service carries the release's image along, so a fetched track
        // without one still gets artwork in the bar.
        let own = playable_from_upcoming(
            &upcoming(Some("1700000000-one.png"), Some("1700000000-album.jpg")),
            "test-artist",
        )
        .expect("a track with audio queues");
        assert_eq!(own.image_src, "/uploads/tracks/1700000000-one.png");

        let album =
            playable_from_upcoming(&upcoming(None, Some("1700000000-album.jpg")), "test-artist")
                .expect("a track with audio queues");
        assert_eq!(album.image_src, "/uploads/releases/1700000000-album.jpg");

        let bare = playable_from_upcoming(&upcoming(None, None), "test-artist")
            .expect("a track with audio queues");
        assert_eq!(bare.image_src, "/Logo.svg", "and the logo at the end");
    }

    #[test]
    fn a_fresh_player_is_closed_and_shows_nothing() {
        let player = PlayerState::default();
        assert!(!player.is_active(), "nothing has been played yet");
        assert_eq!(player.current(), None);
        assert_eq!(player.place(), None, "and there is no place to show");
        assert!(!player.is_playing("slug-one"));
    }

    #[test]
    fn playing_a_queue_opens_the_bar_on_that_track() {
        let mut player = PlayerState::default();
        player.play(queue(&["one", "two", "three"]), 1);

        assert!(player.is_active());
        assert_eq!(
            player.current().map(|track| track.title.as_str()),
            Some("two")
        );
        assert_eq!(player.place(), Some((2, 3)), "the bar shows 2 of 3");
        assert!(player.is_playing("slug-two"));
        assert!(
            !player.is_playing("slug-one"),
            "only the loaded track counts"
        );
    }

    #[test]
    fn a_position_the_queue_does_not_hold_starts_at_the_front() {
        // The list a row was clicked in can change before the click lands; the
        // player starts something rather than opening a bar with nothing in it.
        let mut player = PlayerState::default();
        player.play(queue(&["one", "two"]), 7);
        assert_eq!(
            player.current().map(|track| track.title.as_str()),
            Some("one"),
            "an out-of-range position falls back to the first track"
        );
    }

    #[test]
    fn playing_an_empty_queue_leaves_the_player_closed() {
        let mut player = PlayerState::default();
        player.play(Vec::new(), 0);
        assert!(!player.is_active(), "no track, so no bar");
        assert_eq!(player.current(), None);
    }

    #[test]
    fn next_walks_the_queue_and_holds_at_the_end() {
        let mut player = PlayerState::default();
        player.play(queue(&["one", "two"]), 0);

        assert!(player.next_track(), "one to two");
        assert_eq!(player.place(), Some((2, 2)));
        assert!(
            !player.next_track(),
            "the album finished, so it stays loaded at the end"
        );
        assert_eq!(
            player.current().map(|track| track.title.as_str()),
            Some("two"),
            "and the last track is still the loaded one"
        );
    }

    #[test]
    fn previous_walks_back_and_holds_at_the_front() {
        let mut player = PlayerState::default();
        player.play(queue(&["one", "two"]), 1);

        assert!(player.previous_track(), "two back to one");
        assert!(!player.previous_track(), "nothing before the first track");
        assert_eq!(
            player.current().map(|track| track.title.as_str()),
            Some("one")
        );
    }

    #[test]
    fn transport_buttons_on_a_closed_player_do_nothing() {
        // Guarding the index rather than assuming one: a closed bar can't be
        // stepped through.
        let mut player = PlayerState::default();
        assert!(!player.next_track());
        assert!(!player.previous_track());
        player.close();
        assert!(!player.is_active());
    }

    #[test]
    fn closing_empties_the_queue_for_the_next_play() {
        let mut player = PlayerState::default();
        player.play(queue(&["one", "two", "three"]), 2);
        player.close();

        assert!(!player.is_active());
        assert_eq!(player.current(), None);

        // The same state can be reused afterwards, with a different queue.
        player.play(queue(&["other"]), 0);
        assert_eq!(
            player.current().map(|track| track.title.as_str()),
            Some("other"),
            "a stale queue cannot leak into the next play"
        );
    }

    #[test]
    fn switching_queue_replaces_the_old_list() {
        let mut player = PlayerState::default();
        player.play(queue(&["one", "two", "three"]), 2);
        player.play(queue(&["side"]), 0);

        assert_eq!(player.place(), Some((1, 1)), "the old queue is gone");
        assert!(
            !player.is_playing("slug-three"),
            "a track from the previous release is no longer loaded"
        );
    }

    #[test]
    fn adding_a_track_extends_the_queue_behind_the_loaded_one() {
        let mut player = PlayerState::default();
        player.play(queue(&["one"]), 0);

        assert!(player.enqueue(queue(&["added"])[0].clone()));
        assert_eq!(
            player.place(),
            Some((1, 2)),
            "the loaded track keeps its place"
        );
        assert!(player.next_track(), "the added track steps in");
        assert_eq!(player.place(), Some((2, 2)));
        assert_eq!(
            player.current().map(|track| track.title.as_str()),
            Some("added")
        );
    }

    #[test]
    fn adding_offers_nothing_to_a_closed_player() {
        let mut player = PlayerState::default();
        assert!(
            !player.enqueue(queue(&["one"])[0].clone()),
            "no bar, no queue to extend"
        );
        assert!(!player.is_active());
        assert_eq!(player.place(), None);
    }

    #[test]
    fn the_loaded_track_can_be_added_again() {
        // The rows keep offering `+ Add to Queue` under the playing track
        // itself: listeners replay it deliberately, and duplicates keep the
        // bar's own place rule simple.
        let mut player = PlayerState::default();
        player.play(queue(&["one"]), 0);
        assert!(player.enqueue(queue(&["one"])[0].clone()));
        assert_eq!(player.place(), Some((1, 2)));
    }

    #[test]
    fn a_continuation_batch_extends_the_queue_behind_the_loaded_one() {
        // What a `⏭` at the end, or a track ending there, hands the state:
        // the batch joins the end, the loaded track keeps its place, and the
        // caller's follow-up `next_track` steps onto the batch's first entry.
        let mut player = PlayerState::default();
        player.play(queue(&["one"]), 0);

        assert!(player.append_batch(queue(&["two", "three"])));
        assert_eq!(player.place(), Some((1, 3)), "the loaded track stays put");
        assert!(player.next_track());
        assert_eq!(
            player.current().map(|track| track.title.as_str()),
            Some("two"),
            "the fetch steps the bar onto the first appended track"
        );
        assert!(player.next_track());
        assert_eq!(player.place(), Some((3, 3)));
        assert!(
            !player.append_batch(Vec::new()),
            "an empty batch changes nothing and says so"
        );
        assert_eq!(player.place(), Some((3, 3)));
    }

    #[test]
    fn a_continuation_batch_appends_nothing_to_a_closed_player() {
        // A fetch that lands after the listener closed the bar stops there:
        // no bar, no queue to extend.
        let mut player = PlayerState::default();
        player.play(queue(&["one"]), 0);
        player.close();
        assert!(
            !player.append_batch(queue(&["two"])),
            "the queue went with the bar"
        );
        assert!(!player.is_active());
    }

    #[test]
    fn an_exhausted_catalogue_stops_asking_until_the_next_play() {
        // The empty-answer flag follows one queue, not the player: a new
        // release replayed (or a different one played) gets a fresh look.
        let mut player = PlayerState::default();
        player.play(queue(&["one"]), 0);
        assert!(!player.is_catalogue_exhausted(), "fresh queue, fresh ask");

        player.mark_catalogue_exhausted();
        assert!(player.is_catalogue_exhausted());

        player.play(queue(&["one"]), 0);
        assert!(
            !player.is_catalogue_exhausted(),
            "playing again asks about the artist's catalogue again"
        );
        player.mark_catalogue_exhausted();
        player.close();
        assert!(!player.is_catalogue_exhausted(), "closing clears it too");
    }

    #[test]
    fn queued_slugs_list_the_queue_in_order_duplicates_included() {
        // The exclusion list a continuation fetch leaves out: order matters
        // little, duplicates matter not (`<> ANY` skips the slug however often
        // the queue names it).
        let mut player = PlayerState::default();
        player.play(queue(&["one", "two"]), 0);
        player.enqueue(queue(&["one"])[0].clone());
        assert_eq!(
            player.queued_slugs(),
            vec!["slug-one", "slug-two", "slug-one"]
        );
    }
}
