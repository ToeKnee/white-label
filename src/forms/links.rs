//! Form for editing an artist's music service and social link details.

use crate::models::{music_service::Platform, social_media::SocialMedia};

/// This form is used to collect and update the links associated with an artist, including music streaming services
/// and social media platforms.
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct LinksForm {
    /// The slug of the artist.
    pub artist_slug: String,

    // Music services associated with the artist.
    /// Amazon Music is a music streaming platform and online music store operated by Amazon.
    pub amazon_music: String,
    /// Apple Music is a music streaming service developed by Apple Inc.
    pub apple_music: String,
    /// Bandcamp is a platform for independent musicians to share and sell their music.
    pub bandcamp: String,
    /// This service is primarily used for electronic music and DJ tracks.
    pub beatport: String,
    /// Deezer is a music streaming service that offers a wide range of music tracks.
    pub deezer: String,
    /// `SoundCloud` is a platform for sharing and discovering music.
    pub sound_cloud: String,
    /// Spotify is a popular music streaming service that provides access to a vast library of songs.
    pub spotify: String,
    /// Tidal is a subscription-based music streaming service known for its high-fidelity sound quality.
    pub tidal: String,
    /// `YouTube` Music is a music streaming service developed by `YouTube`, a subsidiary of Google.
    pub you_tube_music: String,

    // Social media services associated with the artist.
    /// `BlueSky` is a decentralized social media platform.
    pub blue_sky: String,
    /// Facebook is a widely used social media platform.
    pub facebook: String,
    /// Instagram is a photo and video sharing social media platform.
    pub instagram: String,
    /// `LinkedIn` is a professional networking platform.
    pub linked_in: String,
    /// Mastodon is a decentralized social media platform.
    pub mastodon: String,
    /// Pinterest is a visual discovery and bookmarking platform.
    pub pinterest: String,
    /// Snapchat is a multimedia messaging app.
    pub snapchat: String,
    /// Threads is a text-based social media platform by Meta.
    pub threads: String,
    /// `TikTok` is a short-form video sharing platform.
    pub tik_tok: String,
    /// Twitter is a microblogging and social networking service.
    pub twitter: String,
    /// `YouTube` is a video sharing platform.
    pub you_tube: String,
}

impl LinksForm {
    /// Take a Platform and return the corresponding link from the form.
    pub fn from_platform(self, platform: &Platform) -> String {
        match platform {
            Platform::AmazonMusic => self.amazon_music,
            Platform::AppleMusic => self.apple_music,
            Platform::Bandcamp => self.bandcamp,
            Platform::Beatport => self.beatport,
            Platform::Deezer => self.deezer,
            Platform::SoundCloud => self.sound_cloud,
            Platform::Spotify => self.spotify,
            Platform::Tidal => self.tidal,
            Platform::YouTubeMusic => self.you_tube_music,
        }
    }

    /// Take a `SocialMedia` and return the corresponding link from the form.
    pub fn from_social_media(self, social_media: &SocialMedia) -> String {
        match social_media {
            SocialMedia::BlueSky => self.blue_sky,
            SocialMedia::Facebook => self.facebook,
            SocialMedia::Instagram => self.instagram,
            SocialMedia::LinkedIn => self.linked_in,
            SocialMedia::Mastodon => self.mastodon,
            SocialMedia::Pinterest => self.pinterest,
            SocialMedia::Snapchat => self.snapchat,
            SocialMedia::Threads => self.threads,
            SocialMedia::TikTok => self.tik_tok,
            SocialMedia::Twitter => self.twitter,
            SocialMedia::YouTube => self.you_tube,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{music_service::Platform, social_media::SocialMedia};

    #[test]
    fn test_from_platform() {
        let form = LinksForm {
            artist_slug: "test_artist".to_string(),
            amazon_music: "amazon".to_string(),
            apple_music: "apple".to_string(),
            bandcamp: "bandcamp".to_string(),
            beatport: "beatport".to_string(),
            deezer: "deezer".to_string(),
            sound_cloud: "soundcloud".to_string(),
            spotify: "spotify".to_string(),
            tidal: "tidal".to_string(),
            you_tube_music: "youtube".to_string(),
            blue_sky: "bluesky".to_string(),
            facebook: "facebook".to_string(),
            instagram: "instagram".to_string(),
            linked_in: "linkedin".to_string(),
            mastodon: "mastodon".to_string(),
            pinterest: "pinterest".to_string(),
            snapchat: "snapchat".to_string(),
            threads: "threads".to_string(),
            tik_tok: "tiktok".to_string(),
            twitter: "twitter".to_string(),
            you_tube: "youtube".to_string(),
        };

        assert_eq!(form.clone().from_platform(&Platform::AmazonMusic), "amazon");
        assert_eq!(form.clone().from_platform(&Platform::AppleMusic), "apple");
        assert_eq!(form.clone().from_platform(&Platform::Bandcamp), "bandcamp");
        assert_eq!(form.clone().from_platform(&Platform::Beatport), "beatport");
        assert_eq!(form.clone().from_platform(&Platform::Deezer), "deezer");
        assert_eq!(
            form.clone().from_platform(&Platform::SoundCloud),
            "soundcloud"
        );
        assert_eq!(form.clone().from_platform(&Platform::Spotify), "spotify");
        assert_eq!(form.clone().from_platform(&Platform::Tidal), "tidal");
        assert_eq!(form.from_platform(&Platform::YouTubeMusic), "youtube");
    }

    #[test]
    fn test_from_social_media() {
        let form = LinksForm {
            artist_slug: "test-artist".to_string(),
            amazon_music: "amazon".to_string(),
            apple_music: "apple".to_string(),
            bandcamp: "bandcamp".to_string(),
            beatport: "beatport".to_string(),
            deezer: "deezer".to_string(),
            sound_cloud: "soundcloud".to_string(),
            spotify: "spotify".to_string(),
            tidal: "tidal".to_string(),
            you_tube_music: "youtube".to_string(),
            blue_sky: "bluesky".to_string(),
            facebook: "facebook".to_string(),
            instagram: "instagram".to_string(),
            linked_in: "linkedin".to_string(),
            mastodon: "mastodon".to_string(),
            pinterest: "pinterest".to_string(),
            snapchat: "snapchat".to_string(),
            threads: "threads".to_string(),
            tik_tok: "tiktok".to_string(),
            twitter: "twitter".to_string(),
            you_tube: "youtube".to_string(),
        };

        assert_eq!(
            form.clone().from_social_media(&SocialMedia::BlueSky),
            "bluesky"
        );
        assert_eq!(
            form.clone().from_social_media(&SocialMedia::Facebook),
            "facebook"
        );
        assert_eq!(
            form.clone().from_social_media(&SocialMedia::Instagram),
            "instagram"
        );
        assert_eq!(
            form.clone().from_social_media(&SocialMedia::LinkedIn),
            "linkedin"
        );
        assert_eq!(
            form.clone().from_social_media(&SocialMedia::Mastodon),
            "mastodon"
        );
        assert_eq!(
            form.clone().from_social_media(&SocialMedia::Pinterest),
            "pinterest"
        );
        assert_eq!(
            form.clone().from_social_media(&SocialMedia::Snapchat),
            "snapchat"
        );
        assert_eq!(
            form.clone().from_social_media(&SocialMedia::Threads),
            "threads"
        );
        assert_eq!(
            form.clone().from_social_media(&SocialMedia::TikTok),
            "tiktok"
        );
        assert_eq!(
            form.clone().from_social_media(&SocialMedia::Twitter),
            "twitter"
        );
        assert_eq!(form.from_social_media(&SocialMedia::YouTube), "youtube");
    }
}
