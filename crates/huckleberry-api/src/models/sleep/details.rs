//! What was recorded about a sleep beyond when it was and how long.
//!
//! Every field here is a flag the app sets or leaves out, so an absent field
//! means "not recorded" rather than "false". The app writes them all present
//! and false when a sleep starts, which is why the `nothing_recorded`
//! constructors exist: writing an empty map instead leaves the app with fields
//! it expects to find missing.

use serde::{Deserialize, Serialize};

/// How the baby went to sleep, or woke up.
///
/// Every field is a flag the app sets or leaves out, so an absent field means
/// "not recorded" rather than "false".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SleepCondition {
    /// Settled happily.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub happy: Option<bool>,
    /// Took a long time to fall asleep.
    #[serde(
        rename = "longTimeToFallAsleep",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub long_time_to_fall_asleep: Option<bool>,
    /// Was upset.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub upset: Option<bool>,
    /// The sleep ended because somebody woke the baby.
    #[serde(
        rename = "wokeUpChild",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub woke_up_child: Option<bool>,
    /// Settled in under ten minutes.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub under_10_minutes: Option<bool>,
    /// Settled in ten to twenty minutes. The key on the wire is
    /// `10-20_minutes`, which is not a Rust identifier and is not a bare
    /// Firestore field path either: see `firestore::field_path`.
    #[serde(
        rename = "10-20_minutes",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub ten_to_twenty_minutes: Option<bool>,
}

impl SleepCondition {
    /// The flags that were recorded true, by their wire names.
    #[must_use]
    pub fn recorded(&self) -> Vec<&'static str> {
        let flags: [(&'static str, Option<bool>); 6] = [
            ("happy", self.happy),
            ("longTimeToFallAsleep", self.long_time_to_fall_asleep),
            ("upset", self.upset),
            ("wokeUpChild", self.woke_up_child),
            ("under_10_minutes", self.under_10_minutes),
            ("10-20_minutes", self.ten_to_twenty_minutes),
        ];
        flags
            .into_iter()
            .filter_map(|(name, set)| (set == Some(true)).then_some(name))
            .collect()
    }

    /// Every flag present and false, which is what the app writes when a sleep
    /// starts and nothing has been chosen yet.
    #[must_use]
    pub const fn nothing_recorded() -> Self {
        Self {
            happy: Some(false),
            long_time_to_fall_asleep: Some(false),
            upset: Some(false),
            woke_up_child: None,
            under_10_minutes: Some(false),
            ten_to_twenty_minutes: Some(false),
        }
    }

    /// The narrower shape the app writes for the *end* of a sleep.
    #[must_use]
    pub const fn nothing_recorded_at_the_end() -> Self {
        Self {
            happy: Some(false),
            long_time_to_fall_asleep: None,
            upset: Some(false),
            woke_up_child: Some(false),
            under_10_minutes: None,
            ten_to_twenty_minutes: None,
        }
    }
}

/// Where the baby slept.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SleepLocations {
    /// In the car.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub car: Option<bool>,
    /// While nursing.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub nursing: Option<bool>,
    /// Worn or held.
    #[serde(
        rename = "wornOrHeld",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub worn_or_held: Option<bool>,
    /// In the stroller.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub stroller: Option<bool>,
    /// Co-sleeping.
    #[serde(
        rename = "coSleep",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub co_sleep: Option<bool>,
    /// Next to a carer.
    #[serde(
        rename = "nextToCarer",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub next_to_carer: Option<bool>,
    /// On their own, in bed.
    #[serde(
        rename = "onOwnInBed",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub on_own_in_bed: Option<bool>,
    /// With a bottle.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub bottle: Option<bool>,
    /// In the swing.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub swing: Option<bool>,
}

impl SleepLocations {
    /// Every place a sleep can be recorded in, by wire name.
    pub const NAMES: &'static [&'static str] = &[
        "car",
        "nursing",
        "wornOrHeld",
        "stroller",
        "coSleep",
        "nextToCarer",
        "onOwnInBed",
        "bottle",
        "swing",
    ];

    /// The places recorded true, by wire name.
    #[must_use]
    pub fn recorded(&self) -> Vec<&'static str> {
        let flags: [(&'static str, Option<bool>); 9] = [
            ("car", self.car),
            ("nursing", self.nursing),
            ("wornOrHeld", self.worn_or_held),
            ("stroller", self.stroller),
            ("coSleep", self.co_sleep),
            ("nextToCarer", self.next_to_carer),
            ("onOwnInBed", self.on_own_in_bed),
            ("bottle", self.bottle),
            ("swing", self.swing),
        ];
        flags
            .into_iter()
            .filter_map(|(name, set)| (set == Some(true)).then_some(name))
            .collect()
    }

    /// Records one place by its wire name, leaving the rest false. Unknown
    /// names are ignored, which is what keeps a caller's typo from silently
    /// becoming a new field on the document.
    #[must_use]
    pub fn only(name: &str) -> Self {
        let mut locations = Self::nothing_recorded();
        let slot = match name {
            "car" => &mut locations.car,
            "nursing" => &mut locations.nursing,
            "wornOrHeld" => &mut locations.worn_or_held,
            "stroller" => &mut locations.stroller,
            "coSleep" => &mut locations.co_sleep,
            "nextToCarer" => &mut locations.next_to_carer,
            "onOwnInBed" => &mut locations.on_own_in_bed,
            "bottle" => &mut locations.bottle,
            "swing" => &mut locations.swing,
            _ => return locations,
        };
        *slot = Some(true);
        locations
    }

    /// Every flag present and false, as the app writes at the start of a
    /// sleep.
    #[must_use]
    pub const fn nothing_recorded() -> Self {
        Self {
            car: Some(false),
            nursing: Some(false),
            worn_or_held: Some(false),
            stroller: Some(false),
            co_sleep: Some(false),
            next_to_carer: Some(false),
            on_own_in_bed: Some(false),
            bottle: Some(false),
            swing: Some(false),
        }
    }
}

/// Everything recorded about one sleep beyond when it was and how long.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SleepDetails {
    /// How the baby went down.
    #[serde(
        rename = "startSleepCondition",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub start_sleep_condition: Option<SleepCondition>,
    /// Where the baby slept.
    #[serde(
        rename = "sleepLocations",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub sleep_locations: Option<SleepLocations>,
    /// How the baby woke.
    #[serde(
        rename = "endSleepCondition",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub end_sleep_condition: Option<SleepCondition>,
    /// Whatever the parent typed.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub notes: Option<String>,
}

impl SleepDetails {
    /// The shape the app writes when a sleep starts: every flag present and
    /// false. Writing an empty map instead leaves the app with fields it
    /// expects to find missing.
    #[must_use]
    pub const fn blank() -> Self {
        Self {
            start_sleep_condition: Some(SleepCondition::nothing_recorded()),
            sleep_locations: Some(SleepLocations::nothing_recorded()),
            end_sleep_condition: Some(SleepCondition::nothing_recorded_at_the_end()),
            notes: None,
        }
    }
}

#[cfg(test)]
mod flags {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_awkward_condition_key_round_trips_under_its_wire_name() {
        let condition: SleepCondition =
            serde_json::from_value(json!({ "10-20_minutes": true })).expect("a condition");
        assert_eq!(condition.ten_to_twenty_minutes, Some(true));
        let text = serde_json::to_string(&condition).expect("serializing");
        assert_eq!(text, r#"{"10-20_minutes":true}"#);
    }

    #[test]
    fn only_the_flags_set_true_are_reported() {
        let locations = SleepLocations::only("wornOrHeld");
        assert_eq!(locations.recorded(), vec!["wornOrHeld"]);
    }

    #[test]
    fn a_place_this_crate_does_not_know_records_nothing_rather_than_inventing_a_field() {
        assert!(SleepLocations::only("hammock").recorded().is_empty());
    }

    #[test]
    fn a_blank_detail_block_writes_every_flag_the_app_expects_to_find() {
        let value = serde_json::to_value(SleepDetails::blank()).expect("serializing");
        assert_eq!(value["sleepLocations"]["onOwnInBed"], json!(false));
        assert_eq!(value["startSleepCondition"]["10-20_minutes"], json!(false));
        assert_eq!(value["endSleepCondition"]["wokeUpChild"], json!(false));
        assert!(
            value["endSleepCondition"].get("under_10_minutes").is_none(),
            "the end block is the narrower shape"
        );
    }
}
