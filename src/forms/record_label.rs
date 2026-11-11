//! Record-label form structs.

/// The label editor's posted fields: the hidden `id` plus the four inputs.
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
    /// Guest plays per track, as posted (see [`LabelFields::parsed_guest_plays`]).
    pub guest_plays: String,
}

/// The most any label may allow, so a mistyped form value cannot make the
/// rule meaningless (and the number stays small enough to render).
const MAX_GUEST_PLAYS: i32 = 99;

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

    /// The guest play allowance as the service takes it: unparsable or blank
    /// falls back to `crate::models::record_label::RecordLabel::DEFAULT_GUEST_PLAYS`,
    /// and anything outside `0..=99` clamps into that range (a guest allowance
    /// over 99 is the same rule for all practical purposes, and `0` means
    /// guests hear nothing).
    #[must_use]
    pub fn parsed_guest_plays(&self) -> i32 {
        self.guest_plays
            .trim()
            .parse()
            .unwrap_or(crate::models::record_label::RecordLabel::DEFAULT_GUEST_PLAYS)
            .clamp(0, MAX_GUEST_PLAYS)
    }
}

#[cfg(test)]
mod tests {
    use super::LabelFields;

    fn posted(guest_plays: &str) -> i32 {
        LabelFields {
            guest_plays: guest_plays.to_string(),
            ..LabelFields::default()
        }
        .parsed_guest_plays()
    }

    #[test]
    fn a_plain_number_is_the_allowance() {
        assert_eq!(posted("3"), 3);
        assert_eq!(posted("0"), 0, "zero switches guests off");
        assert_eq!(posted(" 7 "), 7, "and surrounding spaces are noise");
    }

    #[test]
    fn out_of_range_values_clamp_into_the_readable_range() {
        assert_eq!(posted("-4"), 0, "never below what a guest can get");
        assert_eq!(posted("500"), 99, "and never above what a row can show");
    }

    #[test]
    fn junk_falls_back_to_the_default() {
        assert_eq!(posted(""), 5, "a missing input (or a tampered form)");
        assert_eq!(posted("five"), 5, "and text that was never a number");
    }
}
