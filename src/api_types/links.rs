//! Response types for the artist links endpoints.

use crate::models::{music_service::MusicService, social_media::SocialMediaService};

/// Contains the result of fetching music services for an artist.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct LinksResult {
    /// A vector of music services associated with the artist.
    pub music_services: Vec<MusicService>,
    /// A vector of social links associated with the artist.
    pub social_media_services: Vec<SocialMediaService>,
}
