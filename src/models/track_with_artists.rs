use serde::{Deserialize, Serialize};

use super::{artist::Artist, track::Track};

#[derive(Serialize, Deserialize, Clone, Default, Debug, Eq, PartialEq)]
pub struct TrackWithArtists {
    pub track: Track,
    pub artists: Vec<Artist>,
}
