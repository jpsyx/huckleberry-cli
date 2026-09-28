//! `childs/{cid}`: the child profile, and the two hours that define a night.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::common::{Number, TextOrNumber};

string_enum! {
    /// The gender field on a profile. The app writes an empty string when it
    /// has not been set, which is a value and not an absence.
    Gender {
        Male => "M",
        Female => "F",
        Unspecified => "",
    }
}

/// The nap-prediction state the app keeps on a profile.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ChildSweetspot {
    /// Which nap of the day the prediction is for.
    #[serde(
        rename = "selectedNapDay",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub selected_nap_day: Option<Number>,
    /// The predicted times, keyed by the app's own labels.
    #[serde(
        rename = "sweetSpotTimes",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub sweet_spot_times: Option<BTreeMap<String, Number>>,
    /// The prediction's identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
}

/// `childs/{cid}`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ChildDocument {
    /// The child's name. The app reads this first and falls back to the
    /// account's nickname for the child.
    #[serde(
        rename = "childsName",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub childs_name: Option<String>,
    /// Date of birth, as text (`2025-09-01`) or as an epoch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub birthdate: Option<TextOrNumber>,
    /// When the profile was made.
    #[serde(rename = "createdAt", default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<Number>,
    /// The gender on the profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gender: Option<Gender>,
    /// A Firebase Storage filename.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
    /// The colour the app paints this child in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// When the night begins, as an hour. See [`Self::night_start_hour`].
    #[serde(
        rename = "nightStart",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub night_start: Option<TextOrNumber>,
    /// When the morning begins, as an hour on a 24-hour clock: `6.75` is
    /// 6:45am.
    #[serde(
        rename = "morningCutoff",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub morning_cutoff: Option<TextOrNumber>,
    /// How many naps a day the app expects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub naps: Option<String>,
    /// The nap prediction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sweetspot: Option<ChildSweetspot>,
    /// Whether the child was premature, in weeks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre: Option<Number>,
    /// How many single-entry interval documents exist.
    #[serde(
        rename = "singleIntervalCount",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub single_interval_count: Option<Number>,
    /// When insights were last requested.
    #[serde(
        rename = "lastInsightRequest",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub last_insight_request: Option<Number>,
    /// Which trackers are switched on. A tracker that is off here is one the
    /// account will be refused when it reads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub categories: Option<BTreeMap<String, bool>>,
    /// Which insights have been dismissed.
    #[serde(
        rename = "disabledInsights",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub disabled_insights: Option<BTreeMap<String, bool>>,
    /// How far through the onboarding questionnaire the account is.
    #[serde(
        rename = "questionnaireProgress",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub questionnaire_progress: Option<Number>,
    /// Which app version last completed the questionnaire.
    #[serde(
        rename = "lastQuestionnaireAppVersion",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub last_questionnaire_app_version: Option<String>,
    /// When the questionnaire was last completed.
    #[serde(
        rename = "lastQuestionnaireCompleteTime",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub last_questionnaire_complete_time: Option<Number>,
}

/// The hour the app falls back to when a profile does not say when night
/// begins: 8pm.
pub const DEFAULT_NIGHT_START_HOUR: f64 = 20.0;

/// The hour the app falls back to when a profile does not say when morning
/// begins: 7am.
pub const DEFAULT_MORNING_CUTOFF_HOUR: f64 = 7.0;

impl ChildDocument {
    /// When night begins, as a 24-hour clock hour.
    ///
    /// The profile stores this on a **12-hour** clock: `8.0` is 8pm and `7.5`
    /// is 7:30pm. Anything at or below 12 is therefore a PM hour and is
    /// shifted; anything above is already 24-hour and is left alone. Getting
    /// this wrong puts the whole night window twelve hours out, which is the
    /// difference between "last night" and "this morning".
    #[must_use]
    pub fn night_start_hour(&self) -> f64 {
        let Some(raw) = self.night_start.as_ref().and_then(TextOrNumber::as_f64) else {
            return DEFAULT_NIGHT_START_HOUR;
        };
        if raw <= 12.0 { raw + 12.0 } else { raw }
    }

    /// When morning begins, as a 24-hour clock hour. Already 24-hour on the
    /// profile: `6.75` is 6:45am.
    #[must_use]
    pub fn morning_cutoff_hour(&self) -> f64 {
        self.morning_cutoff
            .as_ref()
            .and_then(TextOrNumber::as_f64)
            .unwrap_or(DEFAULT_MORNING_CUTOFF_HOUR)
    }

    /// Whether a tracker is switched on for this child.
    #[must_use]
    pub fn tracker_enabled(&self, name: &str) -> bool {
        self.categories
            .as_ref()
            .and_then(|categories| categories.get(name))
            .copied()
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod night_boundaries {
    use super::*;

    fn with_hours(night_start: f64, morning_cutoff: f64) -> ChildDocument {
        ChildDocument {
            night_start: Some(TextOrNumber::Number(Number::Float(night_start))),
            morning_cutoff: Some(TextOrNumber::Number(Number::Float(morning_cutoff))),
            ..ChildDocument::default()
        }
    }

    #[test]
    fn eight_on_the_profile_means_eight_in_the_evening() {
        assert!((with_hours(8.0, 7.0).night_start_hour() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn half_past_seven_keeps_its_half() {
        assert!((with_hours(7.5, 7.0).night_start_hour() - 19.5).abs() < f64::EPSILON);
    }

    #[test]
    fn a_value_already_past_noon_is_left_alone() {
        assert!((with_hours(21.0, 7.0).night_start_hour() - 21.0).abs() < f64::EPSILON);
    }

    #[test]
    fn the_morning_cutoff_is_already_a_twenty_four_hour_value() {
        assert!((with_hours(8.0, 6.75).morning_cutoff_hour() - 6.75).abs() < f64::EPSILON);
    }

    #[test]
    fn a_profile_that_says_nothing_gets_the_apps_own_defaults() {
        let bare = ChildDocument::default();
        assert!((bare.night_start_hour() - 20.0).abs() < f64::EPSILON);
        assert!((bare.morning_cutoff_hour() - 7.0).abs() < f64::EPSILON);
    }

    #[test]
    fn an_hour_stored_as_text_is_read_as_a_number() {
        let profile = ChildDocument {
            night_start: Some(TextOrNumber::Text("8".to_owned())),
            ..ChildDocument::default()
        };
        assert!((profile.night_start_hour() - 20.0).abs() < f64::EPSILON);
    }
}

#[cfg(test)]
mod trackers {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_tracker_the_profile_switches_off_reads_as_off() {
        let profile: ChildDocument = serde_json::from_value(json!({
            "childsName": "Bear",
            "categories": { "sleep": true, "solids": false },
        }))
        .expect("a profile");
        assert!(profile.tracker_enabled("sleep"));
        assert!(!profile.tracker_enabled("solids"));
        assert!(!profile.tracker_enabled("activities"), "absent means off");
    }

    #[test]
    fn a_birthdate_stored_as_text_survives_as_text() {
        let profile: ChildDocument =
            serde_json::from_value(json!({ "birthdate": "2025-09-01" })).expect("a profile");
        assert_eq!(
            profile.birthdate.as_ref().map(TextOrNumber::to_text),
            Some("2025-09-01".to_owned())
        );
    }
}
