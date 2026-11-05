//! Response types for the page endpoints.

use crate::models::page::Page;

/// Contains a single Page.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default, Debug)]
pub struct PageResult {
    /// The page being fetched or modified.
    pub page: Page,
}
