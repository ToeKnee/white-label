//! Record-label form structs.

/// The label editor's posted fields: the hidden `id` plus the three inputs.
/// The `id` stays a string here because that is what the browser posts;
/// [`LabelFields::parsed_id`] turns it into what the service takes.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct LabelFields {
    /// The record label's id.
    pub id: String,
    /// The label's display name.
    pub name: String,
    /// The label's description; may contain markdown.
    pub description: String,
    /// The ISRC prefix (country code plus first registrant code).
    pub isrc_base: String,
}

impl LabelFields {
    /// The label id as `crate::services::record_label::update_record_label`
    /// takes it. A tampered or unparsable value becomes 0 and the service
    /// answers with its fixed retry message, which is why the parse is lenient
    /// on purpose. Shared by the javascript-off handler
    /// (`crate::server::forms::update_label`) and the reactive twin.
    #[must_use]
    pub fn parsed_id(&self) -> i64 {
        self.id.parse().unwrap_or_default()
    }
}
