//! Upload configuration details.

use anyhow::{Result, bail};
use std::{fmt, str::FromStr};

/// Represents the details required for file uploads in different locations, with specific configurations for artists, avatars, and releases etc.
#[derive(Debug, Clone)]
pub struct UploadDetails {
    /// The allowed mime types for uploads to this location.
    pub mime_types: Vec<String>,
    /// The path to upload files to, relative to the upload directory.
    pub path: String,
    /// The permisions required for the user to upload to this location.
    pub permissions: Vec<String>,
    /// The maximum file size allowed for uploads to this location.
    pub size_limit: usize,
    /// Should the file be renamed on upload.
    /// If there is already a file with the same name, it will be renamed anyway.
    pub rename: bool,
}

/// Represents the configuration for different upload types. `PartialEq`/`Eq`
/// so it can travel as a dioxus component prop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadConfiguration {
    /// Upload configuration for artist images.
    Artist,
    /// Upload configuration for user avatars.
    Avatar,
    /// Upload configuration for release images.
    Release,
    /// Upload configuration for a track's own audio file.
    TrackAudio,
    /// Upload configuration for a track's own image.
    TrackImage,
}

impl FromStr for UploadConfiguration {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "Artist" => Ok(Self::Artist),
            "Avatar" => Ok(Self::Avatar),
            "Release" => Ok(Self::Release),
            "TrackAudio" => Ok(Self::TrackAudio),
            "TrackImage" => Ok(Self::TrackImage),
            _ => bail!("Invalid upload configuration"),
        }
    }
}

impl UploadConfiguration {
    /// Returns the upload details for the given configuration.
    #[must_use]
    pub fn get_details(&self) -> UploadDetails {
        match self {
            Self::Artist => UploadDetails {
                mime_types: vec![
                    "image/jpeg".to_string(),
                    "image/png".to_string(),
                    "image/gif".to_string(),
                    "image/webp".to_string(),
                ],
                path: "artists".to_string(),
                permissions: vec!["admin".to_string(), "label_owner".to_string()],
                size_limit: 100 * 1024 * 1024, // 100MB
                rename: false,
            },
            Self::Avatar => UploadDetails {
                mime_types: vec![
                    "image/jpeg".to_string(),
                    "image/png".to_string(),
                    "image/gif".to_string(),
                    "image/webp".to_string(),
                ],
                path: "avatars".to_string(),
                permissions: vec![],
                size_limit: 10 * 1024 * 1024, // 10MB
                rename: true,
            },
            Self::Release => UploadDetails {
                mime_types: vec![
                    "image/jpeg".to_string(),
                    "image/png".to_string(),
                    "image/gif".to_string(),
                    "image/webp".to_string(),
                ],
                path: "releases".to_string(),
                permissions: vec!["admin".to_string(), "label_owner".to_string()],
                size_limit: 100 * 1024 * 1024, // 100MB
                rename: false,
            },
            // Track images share the `tracks` directory with track audio; the
            // extension and timestamp prefix keep them apart.
            Self::TrackImage => UploadDetails {
                mime_types: vec![
                    "image/jpeg".to_string(),
                    "image/png".to_string(),
                    "image/gif".to_string(),
                    "image/webp".to_string(),
                ],
                path: "tracks".to_string(),
                permissions: vec!["admin".to_string(), "label_owner".to_string()],
                size_limit: 100 * 1024 * 1024, // 100MB
                rename: false,
            },
            // Audio gets the largest limit of the five: a lossless three-minute
            // track runs to tens of megabytes (a FLAC of a WAV of a CD master is
            // around 30MB), and 250MB leaves room for long or multitrack files.
            // The formats are the four every target browser can play natively;
            // `.m4a`/`.aac` are left out because their `ftyp` box is a brand
            // field rather than a fixed signature, and Safari is the only
            // browser that needs them.
            Self::TrackAudio => UploadDetails {
                mime_types: vec![
                    "audio/mpeg".to_string(),
                    "audio/wav".to_string(),
                    "audio/flac".to_string(),
                    "audio/ogg".to_string(),
                ],
                path: "tracks".to_string(),
                permissions: vec!["admin".to_string(), "label_owner".to_string()],
                size_limit: 250 * 1024 * 1024, // 250MB
                rename: false,
            },
        }
    }
}

impl fmt::Display for UploadConfiguration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
