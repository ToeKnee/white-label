//! The global player: one track playing at a time, kept alive while the user
//! navigates.
//!
//! [`PlayerState`](crate::ui::player::PlayerState) lives in a context that
//! [`crate::ui::shell::App`] provides, so it outlives the router. Its bar,
//! [`GlobalPlayer`](crate::ui::player::GlobalPlayer), is rendered by `AppShell`
//! as page chrome rather than inside a page, which is what keeps the sound
//! going: navigating swaps the `Outlet`'s contents, and the bar's `<audio>`
//! element is never torn down while a track stays loaded.
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

use dioxus::prelude::*;

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
    /// Where the audio lives, already made absolute where the platform needs it
    /// (see [`crate::ui::assets::served_file_url`]).
    pub audio_src: String,
}

/// What the global player has loaded, and where in the queue it stands.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PlayerState {
    /// The tracks previous/next step through.
    queue: Vec<PlayableTrack>,
    /// The loaded track's index in the queue, or `None` while nothing is loaded
    /// (no bar, and each page keeps its own players).
    position: Option<usize>,
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

    rsx! {
        div {
            id: "global-player",
            class: "sticky bottom-0 z-40 border-t bg-base-200 text-base-content",
            div {
                class: "container mx-auto flex flex-wrap items-center gap-2 p-2 not-prose",
                div { class: "min-w-40 flex-1",
                    div { class: "font-semibold", "{current.title}" }
                    div { class: "text-xs opacity-70",
                        "{current.line} · {place.0} of {place.1}"
                    }
                }
                // Rebuilt whenever the loaded track changes (the key), which is
                // what starts the new file playing without touching a track that
                // is already under way. `onended` walks the queue, and stays put
                // at the end of it.
                audio {
                    key: "{current.slug}",
                    class: "w-full min-w-48 max-w-md",
                    controls: true,
                    autoplay: true,
                    src: current.audio_src,
                    onended: move |_| {
                        ended_handler.write().next_track();
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
                        forward_handler.write().next_track();
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
    use super::{PlayableTrack, PlayerState};

    fn queue(names: &[&str]) -> Vec<PlayableTrack> {
        names
            .iter()
            .map(|name| PlayableTrack {
                slug: format!("slug-{name}"),
                title: (*name).to_string(),
                line: "the release".to_string(),
                audio_src: format!("/uploads/tracks/{name}.wav"),
            })
            .collect()
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
}
