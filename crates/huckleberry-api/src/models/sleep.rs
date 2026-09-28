//! `sleep/{cid}`: the running timer, the last sleep, and the history.
//!
//! One unit rule governs this file and is the single easiest thing to get
//! wrong in the whole API: **the sleep timer's `timerStartTime` is in
//! milliseconds**, while the feed timer's is in seconds. A sleep start read as
//! seconds lands in 1970; written as seconds it tells the app the baby has
//! been asleep since the Nixon administration. [`SleepTimer::started_at`]
//! exists so nothing has to remember.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::common::{Number, Timestamp};

/// How the baby went to sleep, or woke up.
///
/// Every field is a flag the app sets or leaves out, so an absent field means
/// "not recorded" rather than "false".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SleepCondition {
    /// Settled happily.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub happy: Option<bool>,
    /// Took a long time to fall asleep.
    #[serde(
        rename = "longTimeToFallAsleep",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub long_time_to_fall_asleep: Option<bool>,
    /// Was upset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upset: Option<bool>,
    /// The sleep ended because somebody woke the baby.
    #[serde(
        rename = "wokeUpChild",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub woke_up_child: Option<bool>,
    /// Settled in under ten minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub under_10_minutes: Option<bool>,
    /// Settled in ten to twenty minutes. The key on the wire is
    /// `10-20_minutes`, which is not a Rust identifier and is not a bare
    /// Firestore field path either: see `firestore::field_path`.
    #[serde(
        rename = "10-20_minutes",
        default,
        skip_serializing_if = "Option::is_none"
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub car: Option<bool>,
    /// While nursing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nursing: Option<bool>,
    /// Worn or held.
    #[serde(
        rename = "wornOrHeld",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub worn_or_held: Option<bool>,
    /// In the stroller.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroller: Option<bool>,
    /// Co-sleeping.
    #[serde(rename = "coSleep", default, skip_serializing_if = "Option::is_none")]
    pub co_sleep: Option<bool>,
    /// Next to a carer.
    #[serde(
        rename = "nextToCarer",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub next_to_carer: Option<bool>,
    /// On their own, in bed.
    #[serde(
        rename = "onOwnInBed",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub on_own_in_bed: Option<bool>,
    /// With a bottle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bottle: Option<bool>,
    /// In the swing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
        skip_serializing_if = "Option::is_none"
    )]
    pub start_sleep_condition: Option<SleepCondition>,
    /// Where the baby slept.
    #[serde(
        rename = "sleepLocations",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub sleep_locations: Option<SleepLocations>,
    /// How the baby woke.
    #[serde(
        rename = "endSleepCondition",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub end_sleep_condition: Option<SleepCondition>,
    /// Whatever the parent typed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
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

/// The nap-prediction analytics the app hangs off a running sleep.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SleepSwsDataShown {
    /// Which nap the first prediction was for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nap_number_a: Option<Number>,
    /// Which nap the second prediction was for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nap_number_b: Option<Number>,
    /// The first predicted time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prediction_time_a: Option<Number>,
    /// The second predicted time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prediction_time_b: Option<Number>,
    /// Where the first prediction came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_a: Option<String>,
    /// Where the second prediction came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_b: Option<String>,
    /// Which wake window the first prediction was for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wake_window_number_a: Option<Number>,
    /// Which wake window the second prediction was for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wake_window_number_b: Option<Number>,
}

/// Analytics attached to a running sleep.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SleepSwsAnalytics {
    /// When the previous sleep ended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_sleep_end_time: Option<Number>,
    /// The previous sleep's interval id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_sleep_interval_key: Option<String>,
    /// What the app showed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sws_data_shown: Option<SleepSwsDataShown>,
    /// When it showed it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<Number>,
}

/// `sleep/{cid}.timer`: the sleep in progress, if there is one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SleepTimer {
    /// Whether a sleep is running. A timer with `active: false` is a finished
    /// session the app has not cleared, not a sleep in progress.
    pub active: bool,
    /// Whether the running sleep is paused.
    pub paused: bool,
    /// When the document last changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<Timestamp>,
    /// The same moment, as a bare number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_timestamp: Option<Number>,
    /// When the sleep started, **in milliseconds**.
    #[serde(
        rename = "timerStartTime",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub timer_start_time: Option<Number>,
    /// When a paused sleep was paused, in milliseconds.
    #[serde(
        rename = "timerEndTime",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub timer_end_time: Option<Number>,
    /// This session's identifier.
    pub uuid: String,
    /// What has been recorded about this sleep so far.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<SleepDetails>,
    /// Nap-prediction analytics.
    #[serde(
        rename = "swsAnalytics",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub sws_analytics: Option<SleepSwsAnalytics>,
}

impl SleepTimer {
    /// When the sleep started, as a Unix timestamp in **seconds**.
    ///
    /// The conversion from the stored milliseconds lives here so that no
    /// caller has to remember the unit.
    #[must_use]
    pub fn started_at(&self) -> Option<f64> {
        self.timer_start_time
            .map(|milliseconds| milliseconds.as_f64() / 1000.0)
    }

    /// When a paused sleep was paused, in seconds.
    #[must_use]
    pub fn paused_at(&self) -> Option<f64> {
        self.timer_end_time
            .map(|milliseconds| milliseconds.as_f64() / 1000.0)
    }

    /// How long the sleep has been running at `now`, in seconds. `None` when
    /// no sleep is running; a paused sleep stops at the moment it was paused.
    #[must_use]
    pub fn elapsed(&self, now: f64) -> Option<f64> {
        if !self.active {
            return None;
        }
        let started = self.started_at()?;
        let until = if self.paused {
            self.paused_at().unwrap_or(now)
        } else {
            now
        };
        Some((until - started).max(0.0))
    }
}

/// The summary of the last completed sleep, kept on the document so the app
/// can show it without reading history.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LastSleep {
    /// When it started, in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<Number>,
    /// How long it lasted, in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<Number>,
    /// The timezone offset in force, in minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<Number>,
}

/// `sleep/{cid}.prefs`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SleepPrefs {
    /// The last completed sleep.
    #[serde(rename = "lastSleep", default, skip_serializing_if = "Option::is_none")]
    pub last_sleep: Option<LastSleep>,
    /// When the preferences last changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<Timestamp>,
    /// The same moment, as a bare number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_timestamp: Option<Number>,
    /// Which nap prediction the app shows.
    #[serde(
        rename = "sweetSpotWhich",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub sweet_spot_which: Option<Number>,
    /// Whether the app notifies about the prediction.
    #[serde(
        rename = "sweetSpotNotify",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub sweet_spot_notify: Option<Number>,
}

/// `sleep/{cid}`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SleepDocument {
    /// The sleep in progress, if there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer: Option<SleepTimer>,
    /// The last completed sleep and the tracker's settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefs: Option<SleepPrefs>,
}

impl SleepDocument {
    /// The running sleep, or `None` when the timer is idle.
    #[must_use]
    pub fn running_timer(&self) -> Option<&SleepTimer> {
        self.timer.as_ref().filter(|timer| timer.active)
    }
}

/// One row of `sleep/{cid}/intervals`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SleepInterval {
    /// The row's own identifier, when it carries one.
    #[serde(rename = "_id", default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// When the sleep started, in seconds.
    pub start: Number,
    /// How long it lasted, in seconds.
    pub duration: Number,
    /// The timezone offset at the start, in minutes.
    pub offset: Number,
    /// The timezone offset at the end, in minutes. Differs from `offset`
    /// across a DST change, which a sleep is quite capable of spanning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_offset: Option<Number>,
    /// What was recorded about the sleep.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<SleepDetails>,
    /// When the row last changed.
    #[serde(
        rename = "lastUpdated",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub last_updated: Option<Number>,
}

impl SleepInterval {
    /// When the sleep ended, in seconds.
    #[must_use]
    pub const fn end(&self) -> f64 {
        self.start.as_f64() + self.duration.as_f64()
    }
}

/// A batch of sleep rows.
pub type SleepMultiContainer = super::common::MultiContainer<SleepInterval>;

/// Sleep rows keyed by id, as a batch stores them.
pub type SleepRows = BTreeMap<String, SleepInterval>;

#[cfg(test)]
mod timers {
    use super::*;

    fn running(start_milliseconds: f64) -> SleepTimer {
        SleepTimer {
            active: true,
            paused: false,
            timestamp: None,
            local_timestamp: None,
            timer_start_time: Some(Number::Float(start_milliseconds)),
            timer_end_time: None,
            uuid: "0123456789abcdef".to_owned(),
            details: None,
            sws_analytics: None,
        }
    }

    #[test]
    fn the_start_is_milliseconds_and_comes_back_as_seconds() {
        let timer = running(1_758_572_400_000.0);
        assert!((timer.started_at().expect("a start") - 1_758_572_400.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_running_sleep_counts_up() {
        let timer = running(1_758_572_400_000.0);
        assert!((timer.elapsed(1_758_576_000.0).expect("elapsed") - 3600.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_paused_sleep_stops_at_the_moment_it_was_paused() {
        let mut timer = running(1_758_572_400_000.0);
        timer.paused = true;
        timer.timer_end_time = Some(Number::Float(1_758_574_200_000.0));
        assert!((timer.elapsed(1_758_576_000.0).expect("elapsed") - 1800.0).abs() < f64::EPSILON);
    }

    #[test]
    fn an_inactive_timer_is_not_a_sleep_in_progress() {
        let mut timer = running(1_758_572_400_000.0);
        timer.active = false;
        assert!(timer.elapsed(1_758_576_000.0).is_none());
    }

    #[test]
    fn a_clock_that_has_gone_backwards_reads_as_zero_rather_than_negative() {
        let timer = running(1_758_572_400_000.0);
        assert!((timer.elapsed(1_758_572_000.0).expect("elapsed")).abs() < f64::EPSILON);
    }

    #[test]
    fn a_document_with_an_idle_timer_reports_no_running_sleep() {
        let mut timer = running(1.0);
        timer.active = false;
        let document = SleepDocument {
            timer: Some(timer),
            prefs: None,
        };
        assert!(document.running_timer().is_none());
    }
}

#[cfg(test)]
mod details {
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

#[cfg(test)]
mod intervals {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_row_knows_when_it_ended() {
        let interval: SleepInterval =
            serde_json::from_value(json!({ "start": 1000, "duration": 3600, "offset": -240 }))
                .expect("a row");
        assert!((interval.end() - 4600.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_batch_carries_its_rows_under_their_own_keys() {
        let container: SleepMultiContainer = serde_json::from_value(json!({
            "multi": true,
            "hasMoreRoom": true,
            "data": {
                "row-one": { "start": 1000, "duration": 60, "offset": -240 },
            },
        }))
        .expect("a batch");
        assert_eq!(container.data.len(), 1);
        assert!(container.data.contains_key("row-one"));
    }
}
