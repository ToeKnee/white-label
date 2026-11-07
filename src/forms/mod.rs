//! This module contains structs represnting data used in web forms.
//!
//! Two shapes live side by side. The service forms (`CreateReleaseForm`, …)
//! carry typed values and are what `crate::services` takes; the posted-field
//! forms (`ReleaseCreateFields`, …) carry exactly what a browser posts — every
//! value a string — and each turns itself into the service form next door with
//! `into_service_form`. Both the javascript-off handlers
//! (`crate::server::forms`) and the reactive twins (`crate::ui`) build their
//! service form that way, so the two paths validate identically.

pub mod artist;
pub mod auth;
pub mod links;
pub mod page;
pub mod record_label;
pub mod release;
pub mod track;
pub mod user;

use chrono::{DateTime, NaiveDateTime, Utc};

/// The hidden slug carried by the page and artist delete/restore forms.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct SlugFields {
    /// The slug of the row to soft-delete or restore.
    pub slug: String,
}

/// Resolve a `DateField` submission.
///
/// A submitted `datetime-local` value wins (interpreted as UTC — the browser's
/// zone is not applied; see `crate::ui::admin::pages`), else the hidden
/// full-date field, else no date.
#[must_use]
pub fn parse_form_date(local: &str, hidden: &str) -> Option<DateTime<Utc>> {
    if !local.is_empty() {
        // `datetime-local` sends minute precision; some browsers append
        // seconds (or fractions).
        for format in ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M"] {
            if let Ok(naive) = NaiveDateTime::parse_from_str(local, format) {
                return Some(naive.and_utc());
            }
        }
    }
    if !hidden.is_empty() {
        // The hidden field carries `DateTime<Utc>`'s Display form.
        if let Ok(when) = DateTime::parse_from_rfc3339(hidden) {
            return Some(when.to_utc());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::parse_form_date;
    use crate::forms::auth::{ClientPasswordFields, ClientProfileFields};
    use crate::forms::release::ReleaseUpdateFields;
    use crate::forms::track::TrackUpdateFields;
    use chrono::{TimeZone, Utc};

    #[test]
    fn form_date_prefers_the_local_input() {
        let parsed =
            parse_form_date("2026-03-01T09:30", "2020-01-01T00:00:00+00:00").expect("date");
        assert_eq!(parsed, Utc.with_ymd_and_hms(2026, 3, 1, 9, 30, 0).unwrap());
    }

    #[test]
    fn form_date_accepts_seconds_in_the_local_input() {
        let parsed = parse_form_date("2026-03-01T09:30:15", "").expect("date");
        assert_eq!(parsed, Utc.with_ymd_and_hms(2026, 3, 1, 9, 30, 15).unwrap());
    }

    #[test]
    fn form_date_falls_back_to_the_hidden_field() {
        let parsed = parse_form_date("", "2026-02-15T12:00:00+00:00").expect("date");
        assert_eq!(parsed, Utc.with_ymd_and_hms(2026, 2, 15, 12, 0, 0).unwrap());
        // An unparseable local value also falls through.
        let parsed = parse_form_date("not-a-date", "2026-02-15T12:00:00+00:00").expect("date");
        assert_eq!(parsed, Utc.with_ymd_and_hms(2026, 2, 15, 12, 0, 0).unwrap());
    }

    #[test]
    fn form_date_empty_both_means_no_date() {
        assert!(parse_form_date("", "").is_none());
        assert!(parse_form_date("garbage", "garbage").is_none());
    }

    #[test]
    fn release_fields_turn_into_the_service_form() {
        let fields = ReleaseUpdateFields {
            slug: "a-release".into(),
            artist_ids: "1,2".into(),
            label_id: "7".into(),
            name: "A Release".into(),
            description: "notes".into(),
            catalogue_number: "CAT1".into(),
            primary_artist_id: "2".into(),
            release_date: String::new(),
            published_at: String::new(),
            local_release_date: "2026-03-01T09:30".into(),
            local_published_at: String::new(),
            artist_slug: String::new(),
        };
        let form = fields.into_service_form();
        assert_eq!(form.artist_ids, "1,2");
        assert_eq!(form.slug, "a-release");
        assert_eq!(form.label_id, 7);
        assert_eq!(form.primary_artist_id, 2);
        assert_eq!(
            form.release_date,
            Some(Utc.with_ymd_and_hms(2026, 3, 1, 9, 30, 0).unwrap())
        );
        assert_eq!(form.published_at, None);
    }

    #[test]
    fn unparsable_numbers_in_posted_fields_become_defaults() {
        // A hand-crafted body gets the service's own validation rather than an
        // invented id.
        let fields = ReleaseUpdateFields {
            label_id: "<script>".into(),
            primary_artist_id: String::new(),
            ..ReleaseUpdateFields::default()
        };
        let form = fields.into_service_form();
        assert_eq!(form.label_id, 0);
        assert_eq!(form.primary_artist_id, 0);

        let track = TrackUpdateFields {
            bpm: "fast".into(),
            track_number: "first".into(),
            ..TrackUpdateFields::default()
        }
        .into_service_form();
        assert_eq!(track.bpm, None);
        assert_eq!(track.track_number, 0);
        // A blank ISRC stays Some(""): the column is NOT NULL and the model's
        // validate step reports the short code.
        assert_eq!(track.isrc_code, Some(String::new()));
    }

    #[test]
    fn profile_fields_turn_blank_inputs_into_no_value() {
        let filled = ClientProfileFields {
            original_username: "alice".into(),
            username: "alice".into(),
            email: "alice@example.com".into(),
            description: "hello".into(),
            first_name: "Alice".into(),
            last_name: "Angela".into(),
        };
        let form = filled.into_service_form();
        assert_eq!(form.original_username, "alice");
        assert_eq!(form.description, Some("hello".into()));
        assert_eq!(form.first_name, Some("Alice".into()));
        assert_eq!(form.last_name, Some("Angela".into()));

        // The browser posts every pre-filled input, so an emptied one arrives
        // as "" and has to mean "no value" rather than Some("").
        let cleared = ClientProfileFields {
            original_username: "alice".into(),
            username: "alice".into(),
            email: "alice@example.com".into(),
            ..ClientProfileFields::default()
        };
        let form = cleared.into_service_form();
        assert_eq!(form.description, None);
        assert_eq!(form.first_name, None);
        assert_eq!(form.last_name, None);
    }

    #[test]
    fn posted_auth_fields_tolerate_a_hand_crafted_body() {
        // A missing input defaults rather than failing the request, so the
        // service's own validation reports what is wrong.
        let profile: ClientProfileFields =
            serde_json::from_value(serde_json::json!({"username": "alice"}))
                .expect("profile fields");
        assert_eq!(profile.original_username, "");
        assert_eq!(profile.email, "");

        let password: ClientPasswordFields =
            serde_json::from_value(serde_json::json!({"new_password": "fresh123"}))
                .expect("password fields");
        assert_eq!(password.password, "");
        assert_eq!(password.new_password, "fresh123");
    }
}
