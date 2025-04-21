use anyhow::{Result, bail};
use std::{fmt, str::FromStr};

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

#[derive(Debug, Clone, Copy)]
pub enum UploadConfiguration {
    Artist,
    Avatar,
    Release,
}

impl FromStr for UploadConfiguration {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "Artist" => Ok(Self::Artist),
            "Avatar" => Ok(Self::Avatar),
            "Release" => Ok(Self::Release),
            _ => bail!("Invalid upload configuration"),
        }
    }
}

impl UploadConfiguration {
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
        }
    }
}

impl fmt::Display for UploadConfiguration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
