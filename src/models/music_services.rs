//! This module defines varous music services

use serde::{Deserialize, Serialize};

/// Enum representing different music services.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub enum MusicService {
    /// Amazon Music service.
    /// Amazon Music is a music streaming platform and online music store operated by Amazon.
    AmazonMusic,
    /// Apple Music service.
    /// Apple Music is a music streaming service developed by Apple Inc.
    AppleMusic,
    /// Bandcamp music service.
    /// Bandcamp is a platform for independent musicians to share and sell their music.
    Bandcamp,
    /// Beatport music service.
    /// This service is primarily used for electronic music and DJ tracks.
    Beatport,
    /// Deezer music service.
    /// Deezer is a music streaming service that offers a wide range of music tracks.
    Deezer,
    /// SoundCloud music service.
    /// SoundCloud is a platform for sharing and discovering music.
    SoundCloud,
    /// Spotify music service.
    /// Spotify is a popular music streaming service that provides access to a vast library of songs.
    Spotify,
    /// Tidal music service.
    /// Tidal is a subscription-based music streaming service known for its high-fidelity sound quality.
    Tidal,
    /// YouTube Music service.
    /// YouTube Music is a music streaming service developed by YouTube, a subsidiary of Google.
    YouTubeMusic,
}
