//! Response types for the record label endpoints.

use crate::models::{artist::Artist, page::Page, record_label::RecordLabel};

/// A result containing a single `RecordLabel`.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct LabelResult {
    /// The record label being returned.
    pub record_label: RecordLabel,
}

/// A result containing a list of artists associated with a `RecordLabel`.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct LabelArtistResult {
    /// A vector of artists associated with the record label.
    pub artists: Vec<Artist>,
}

/// A result containing a list of pages associated with a `RecordLabel`.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct LabelPageResult {
    /// A vector of pages associated with the record label.
    pub pages: Vec<Page>,
}
