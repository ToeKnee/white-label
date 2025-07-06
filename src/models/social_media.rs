//! This module defines an enumeration for various social media platforms.
use serde::{Deserialize, Serialize};

/// SocialMedia is an enumeration representing various social media platforms.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub enum SocialMedia {
    /// BlueSky is a decentralized social media platform.
    BlueSky,
    /// Facebook is a widely used social media platform.
    Facebook,
    /// Instagram is a photo and video sharing social media platform.
    Instagram,
    /// LinkedIn is a professional networking platform.
    LinkedIn,
    /// Mastodon is a decentralized social media platform.
    Mastodon,
    /// Pinterest is a visual discovery and bookmarking platform.
    Pinterest,
    /// Snapchat is a multimedia messaging app.
    Snapchat,
    /// Threads is a text-based social media platform by Meta.
    Threads,
    /// TikTok is a short-form video sharing platform.
    TikTok,
    /// Twitter is a microblogging and social networking service.
    Twitter,
    /// YouTube is a video sharing platform.
    YouTube,
}
