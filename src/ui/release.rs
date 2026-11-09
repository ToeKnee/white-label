//! The public release page.
//!
//! Album details plus the track list, each track showing its featured artists
//! and, when it has an audio file, the browser's own player.
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
            TrackList { tracks }
        }
    }
}

/// The tracks on a release, or nothing when there are none.
#[component]
fn TrackList(
    /// The tracks to display
    tracks: Vec<TrackWithArtists>,
) -> Element {
    if tracks.is_empty() {
        return rsx! {};
    }
    rsx! {
        h2 { "Tracks" }
        ul { class: "list bg-base-100",
            for track in tracks {
                Track {
                    key: "{track.track.slug}",
                    track,
                }
            }
        }
    }
}

/// One track row: thumbnail, name, featured artists, shortened blurb, and a
/// player when the track has an audio file.
#[component]
fn Track(
    /// The track to display
    track: TrackWithArtists,
) -> Element {
    let featured = featured_artist_names(&track);
    let blurb = shorten_string(track.track.description.clone());
    let player_source = audio_source(&track);
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
                // The browser's own player. Uploaded audio lives under
                // `/uploads`, which the server serves to anyone, so a guest
                // hears the same track a signed-in visitor does.
                if let Some(source) = player_source {
                    audio {
                        class: "w-full not-prose",
                        controls: true,
                        src: source,
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

/// Names of artists who contributed to the track, excluding the primary
/// artist (the `FeaturedTrackArtists` component's logic; extracted as a
/// pure function so the empty case and the join are testable).
#[must_use]
fn featured_artist_names(track: &TrackWithArtists) -> String {
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
    use super::{audio_source, featured_artist_names};
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
}
