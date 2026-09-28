//! `feed/{cid}`: nursing, bottles and solids all live in one tracker.
//!
//! Three things in this file routinely surprise people:
//!
//! - **The feed timer's `timerStartTime` is in seconds**, unlike the sleep
//!   timer's milliseconds, and it is not the start of the feed. It is the
//!   start of the *current side*, and it resets every time the side is
//!   switched or the timer is resumed. `feedStartTime` is the start of the
//!   feed.
//! - **`lastSide` can be the literal string `"none"`** while a switch or a
//!   resume is in flight. It is a transition marker, not "no side".
//! - **A bottle row and the `prefs.lastBottle` summary name the same two
//!   numbers differently**: the row says `amount`/`units`, the summary says
//!   `bottleAmount`/`bottleUnits`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::common::{Number, ReminderV2, Timestamp};
use super::solids::{SolidsFoodEntry, SolidsReaction};

string_enum! {
    /// Which breast.
    FeedSide {
        Left => "left",
        Right => "right",
        /// A transition marker the app writes between sides, not an absence.
        None => "none",
    }
}

string_enum! {
    /// What was in the bottle.
    BottleType {
        BreastMilk => "Breast Milk",
        Formula => "Formula",
        TubeFeeding => "Tube Feeding",
        CowMilk => "Cow Milk",
        GoatMilk => "Goat Milk",
        SoyMilk => "Soy Milk",
        Other => "Other",
    }
}

string_enum! {
    /// How a volume is measured.
    VolumeUnits {
        Millilitres => "ml",
        Ounces => "oz",
    }
}

/// How many millilitres are in a fluid ounce, US customary, which is the one
/// the app means.
pub const MILLILITRES_PER_OUNCE: f64 = 29.573_529_562_5;

impl VolumeUnits {
    /// Converts an amount in these units to millilitres, which is the unit
    /// every total in this crate is kept in.
    #[must_use]
    pub fn to_millilitres(&self, amount: f64) -> f64 {
        match self {
            Self::Ounces => amount * MILLILITRES_PER_OUNCE,
            // An unfamiliar unit is taken at face value rather than guessed
            // at: inventing a conversion factor would be worse than none.
            Self::Millilitres | Self::Unknown(_) => amount,
        }
    }
}

/// The summary of the last nursing session.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LastNursing {
    /// Always `breast`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// When it started, in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<Number>,
    /// Both sides together, in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<Number>,
    /// The left side, in seconds.
    #[serde(
        rename = "leftDuration",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub left_duration: Option<Number>,
    /// The right side, in seconds.
    #[serde(
        rename = "rightDuration",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub right_duration: Option<Number>,
    /// The timezone offset in force, in minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<Number>,
}

/// Which side the last feed finished on, so the app can suggest the other.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LastSide {
    /// When that feed started.
    pub start: Number,
    /// Which side it ended on.
    #[serde(rename = "lastSide")]
    pub last_side: FeedSide,
}

/// The summary of the last bottle.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LastBottle {
    /// Always `bottle`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// When it was given, in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<Number>,
    /// What was in it.
    #[serde(
        rename = "bottleType",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub bottle_type: Option<BottleType>,
    /// How much. Named differently from the interval row's `amount`.
    #[serde(
        rename = "bottleAmount",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub bottle_amount: Option<Number>,
    /// In what units. Named differently from the interval row's `units`.
    #[serde(
        rename = "bottleUnits",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub bottle_units: Option<VolumeUnits>,
    /// The timezone offset in force, in minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<Number>,
}

/// The summary of the last solids entry.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LastSolid {
    /// Always `solids`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// When it was, in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<Number>,
    /// What was eaten, keyed by food id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foods: Option<BTreeMap<String, SolidsFoodEntry>>,
    /// How it went.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reactions: Option<BTreeMap<String, bool>>,
    /// Whatever the parent typed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// The timezone offset in force, in minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<Number>,
}

/// `feed/{cid}.prefs`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FeedPrefs {
    /// The bottle contents the app offers by default: whatever was last used.
    #[serde(
        rename = "bottleType",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub bottle_type: Option<BottleType>,
    /// The amount the app offers by default.
    #[serde(
        rename = "bottleAmount",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub bottle_amount: Option<Number>,
    /// The units the app offers by default.
    #[serde(
        rename = "bottleUnits",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub bottle_units: Option<VolumeUnits>,
    /// The last bottle.
    #[serde(
        rename = "lastBottle",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub last_bottle: Option<LastBottle>,
    /// That bottle's session id.
    #[serde(
        rename = "lastBottleUuid",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub last_bottle_uuid: Option<String>,
    /// The last nursing session.
    #[serde(
        rename = "lastNursing",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub last_nursing: Option<LastNursing>,
    /// That session's id.
    #[serde(
        rename = "lastNursingUuid",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub last_nursing_uuid: Option<String>,
    /// Which side to offer next.
    #[serde(rename = "lastSide", default, skip_serializing_if = "Option::is_none")]
    pub last_side: Option<LastSide>,
    /// The last solids entry.
    #[serde(rename = "lastSolid", default, skip_serializing_if = "Option::is_none")]
    pub last_solid: Option<LastSolid>,
    /// The feeding reminder.
    #[serde(
        rename = "reminderV2",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub reminder: Option<ReminderV2>,
    /// The solids reminder.
    #[serde(
        rename = "solids_reminderV2",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub solids_reminder: Option<ReminderV2>,
    /// When the preferences last changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<Timestamp>,
    /// The same moment, as a bare number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_timestamp: Option<Number>,
}

/// `feed/{cid}.timer`: the nursing session in progress, if there is one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedTimer {
    /// Whether a session is running.
    pub active: bool,
    /// Whether it is paused.
    pub paused: bool,
    /// When the document last changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<Timestamp>,
    /// The same moment, as a bare number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_timestamp: Option<Number>,
    /// When the whole feed started, in seconds.
    #[serde(
        rename = "feedStartTime",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub feed_start_time: Option<Number>,
    /// When the current side started, in seconds. Resets on switch and resume.
    #[serde(
        rename = "timerStartTime",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub timer_start_time: Option<Number>,
    /// This session's identifier.
    pub uuid: String,
    /// Seconds banked on the left so far.
    #[serde(
        rename = "leftDuration",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub left_duration: Option<Number>,
    /// Seconds banked on the right so far.
    #[serde(
        rename = "rightDuration",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub right_duration: Option<Number>,
    /// The side last fed on, or `none` mid-transition.
    #[serde(rename = "lastSide", default, skip_serializing_if = "Option::is_none")]
    pub last_side: Option<FeedSide>,
    /// The side being fed on now.
    #[serde(
        rename = "activeSide",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub active_side: Option<FeedSide>,
}

impl FeedTimer {
    /// Which side is being fed on, falling back the way the app does:
    /// the active side, then the last side, then the left.
    #[must_use]
    pub fn current_side(&self) -> FeedSide {
        self.active_side
            .clone()
            .or_else(|| self.last_side.clone())
            .unwrap_or(FeedSide::Left)
    }

    /// Seconds banked on each side, not counting the side running now.
    #[must_use]
    pub fn banked(&self) -> (f64, f64) {
        (
            self.left_duration.map_or(0.0, Number::as_f64),
            self.right_duration.map_or(0.0, Number::as_f64),
        )
    }

    /// Seconds on each side at `now`, including the side running now.
    ///
    /// A paused session banks nothing further, which is what makes pausing
    /// and completing add up to the same total as completing outright.
    #[must_use]
    pub fn totals(&self, now: f64) -> (f64, f64) {
        let (mut left, mut right) = self.banked();
        if self.paused {
            return (left, right);
        }
        let started = self.timer_start_time.map_or(now, Number::as_f64);
        let elapsed = (now - started).max(0.0);
        if self.current_side() == FeedSide::Right {
            right += elapsed;
        } else {
            left += elapsed;
        }
        (left, right)
    }

    /// How long the whole feed has run at `now`, in seconds. `None` when no
    /// session is running.
    #[must_use]
    pub fn elapsed(&self, now: f64) -> Option<f64> {
        if !self.active {
            return None;
        }
        let (left, right) = self.totals(now);
        Some(left + right)
    }
}

/// `feed/{cid}`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FeedDocument {
    /// The nursing session in progress, if there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer: Option<FeedTimer>,
    /// The last feed of each kind, and the tracker's settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefs: Option<FeedPrefs>,
}

impl FeedDocument {
    /// The running nursing session, or `None` when the timer is idle.
    ///
    /// The distinction matters more here than for sleep: a finished feed is
    /// left as `{ active: false, paused: true }`, which reads at a glance like
    /// a paused feed in progress and is not one.
    #[must_use]
    pub fn running_timer(&self) -> Option<&FeedTimer> {
        self.timer.as_ref().filter(|timer| timer.active)
    }
}

/// One nursing row of `feed/{cid}/intervals`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BreastFeedInterval {
    /// When the feed started, in seconds.
    pub start: Number,
    /// Which side it ended on.
    #[serde(rename = "lastSide")]
    pub last_side: FeedSide,
    /// When the row last changed.
    #[serde(
        rename = "lastUpdated",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub last_updated: Option<Number>,
    /// Seconds on the left.
    #[serde(rename = "leftDuration")]
    pub left_duration: Number,
    /// Seconds on the right.
    #[serde(rename = "rightDuration")]
    pub right_duration: Number,
    /// The timezone offset at the start, in minutes.
    pub offset: Number,
    /// The timezone offset at the end, in minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_offset: Option<Number>,
    /// Whatever the parent typed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl BreastFeedInterval {
    /// Both sides together, in seconds.
    #[must_use]
    pub const fn total_seconds(&self) -> f64 {
        self.left_duration.as_f64() + self.right_duration.as_f64()
    }
}

/// One bottle row of `feed/{cid}/intervals`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BottleFeedInterval {
    /// When the bottle was given, in seconds.
    pub start: Number,
    /// When the row last changed.
    #[serde(
        rename = "lastUpdated",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub last_updated: Option<Number>,
    /// What was in it.
    #[serde(rename = "bottleType")]
    pub bottle_type: BottleType,
    /// How much.
    pub amount: Number,
    /// In what units.
    pub units: VolumeUnits,
    /// The timezone offset at the start, in minutes.
    pub offset: Number,
    /// The timezone offset at the end, in minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_offset: Option<Number>,
    /// Whatever the parent typed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl BottleFeedInterval {
    /// How much was taken, in millilitres, whatever it was recorded in.
    #[must_use]
    pub fn millilitres(&self) -> f64 {
        self.units.to_millilitres(self.amount.as_f64())
    }
}

/// One solids row of `feed/{cid}/intervals`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SolidsFeedInterval {
    /// When the meal was, in seconds.
    pub start: Number,
    /// When the row last changed.
    #[serde(
        rename = "lastUpdated",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub last_updated: Option<Number>,
    /// The timezone offset, in minutes.
    pub offset: Number,
    /// What was eaten, keyed by food id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foods: Option<BTreeMap<String, SolidsFoodEntry>>,
    /// How it went.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reactions: Option<BTreeMap<String, bool>>,
    /// Whatever the parent typed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// A Firebase Storage filename for a photo of the meal.
    #[serde(
        rename = "foodNoteImage",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub food_note_image: Option<String>,
    /// The batch this row came out of, when it came out of one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multientry_key: Option<String>,
    /// The timezone offset at the end, in minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_offset: Option<Number>,
}

impl SolidsFeedInterval {
    /// How the baby took it, if it was recorded.
    #[must_use]
    pub fn reaction(&self) -> Option<SolidsReaction> {
        self.reactions.as_ref().and_then(|reactions| {
            reactions
                .iter()
                .find(|(_, recorded)| **recorded)
                .map(|(name, _)| SolidsReaction::from_wire(name))
        })
    }
}

/// One row of `feed/{cid}/intervals`, whichever kind of feed it was.
///
/// The `mode` field is the discriminator, which is how the app itself tells
/// them apart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum FeedInterval {
    /// A nursing session.
    Breast(BreastFeedInterval),
    /// A bottle.
    Bottle(BottleFeedInterval),
    /// A meal.
    Solids(SolidsFeedInterval),
}

impl FeedInterval {
    /// When the feed was, in seconds.
    #[must_use]
    pub const fn start(&self) -> f64 {
        match self {
            Self::Breast(feed) => feed.start.as_f64(),
            Self::Bottle(feed) => feed.start.as_f64(),
            Self::Solids(feed) => feed.start.as_f64(),
        }
    }

    /// Whatever the parent typed.
    #[must_use]
    pub fn notes(&self) -> Option<&str> {
        match self {
            Self::Breast(feed) => feed.notes.as_deref(),
            Self::Bottle(feed) => feed.notes.as_deref(),
            Self::Solids(feed) => feed.notes.as_deref(),
        }
    }
}

/// A batch of feed rows.
pub type FeedMultiContainer = super::common::MultiContainer<FeedInterval>;

#[cfg(test)]
mod units {
    use super::*;

    #[test]
    fn ounces_convert_and_millilitres_do_not() {
        let four_ounces = VolumeUnits::Ounces.to_millilitres(4.0);
        assert!((four_ounces - 118.294_118_25).abs() < 0.001);
        assert!((VolumeUnits::Millilitres.to_millilitres(120.0) - 120.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_unit_this_crate_does_not_know_is_taken_at_face_value() {
        let unknown = VolumeUnits::Unknown("cc".to_owned());
        assert!((unknown.to_millilitres(30.0) - 30.0).abs() < f64::EPSILON);
    }
}

#[cfg(test)]
mod timers {
    use super::*;

    fn nursing(side: FeedSide, banked: (f64, f64), started: f64) -> FeedTimer {
        FeedTimer {
            active: true,
            paused: false,
            timestamp: None,
            local_timestamp: None,
            feed_start_time: Some(Number::Float(started)),
            timer_start_time: Some(Number::Float(started)),
            uuid: "0123456789abcdef".to_owned(),
            left_duration: Some(Number::Float(banked.0)),
            right_duration: Some(Number::Float(banked.1)),
            last_side: Some(FeedSide::Left),
            active_side: Some(side),
        }
    }

    #[test]
    fn the_running_side_accrues_and_the_other_does_not() {
        let timer = nursing(FeedSide::Right, (300.0, 0.0), 1000.0);
        let (left, right) = timer.totals(1120.0);
        assert!((left - 300.0).abs() < f64::EPSILON);
        assert!((right - 120.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_paused_session_banks_nothing_further() {
        let mut timer = nursing(FeedSide::Right, (300.0, 60.0), 1000.0);
        timer.paused = true;
        assert_eq!(timer.totals(9999.0), (300.0, 60.0));
    }

    #[test]
    fn the_side_falls_back_the_way_the_app_does() {
        let mut timer = nursing(FeedSide::Right, (0.0, 0.0), 1000.0);
        assert_eq!(timer.current_side(), FeedSide::Right);
        timer.active_side = None;
        assert_eq!(
            timer.current_side(),
            FeedSide::Left,
            "falls back to lastSide"
        );
        timer.last_side = None;
        assert_eq!(timer.current_side(), FeedSide::Left, "and then to the left");
    }

    #[test]
    fn an_inactive_timer_is_not_a_feed_in_progress_even_when_it_looks_paused() {
        // The shape a completed feed leaves behind.
        let mut timer = nursing(FeedSide::Left, (600.0, 0.0), 1000.0);
        timer.active = false;
        timer.paused = true;
        let document = FeedDocument {
            timer: Some(timer),
            prefs: None,
        };
        assert!(document.running_timer().is_none());
    }
}

#[cfg(test)]
mod intervals {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_bottle_row_decodes_by_its_mode() {
        let row: FeedInterval = serde_json::from_value(json!({
            "mode": "bottle",
            "start": 1_758_572_400,
            "bottleType": "Formula",
            "amount": 90.0,
            "units": "ml",
            "offset": -240.0,
        }))
        .expect("a bottle row");
        let FeedInterval::Bottle(bottle) = &row else {
            panic!("expected a bottle, got {row:?}");
        };
        assert!((bottle.millilitres() - 90.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_bottle_recorded_in_ounces_still_totals_in_millilitres() {
        let row: BottleFeedInterval = serde_json::from_value(json!({
            "start": 1, "bottleType": "Breast Milk", "amount": 3, "units": "oz", "offset": 0,
        }))
        .expect("a bottle row");
        assert!((row.millilitres() - 88.720_588_687_5).abs() < 0.001);
    }

    #[test]
    fn a_nursing_row_adds_its_two_sides() {
        let row: BreastFeedInterval = serde_json::from_value(json!({
            "start": 1, "lastSide": "right", "leftDuration": 300.0,
            "rightDuration": 420.0, "offset": 0,
        }))
        .expect("a nursing row");
        assert!((row.total_seconds() - 720.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_solids_row_reports_the_reaction_that_was_recorded() {
        let row: SolidsFeedInterval = serde_json::from_value(json!({
            "start": 1, "offset": 0, "reactions": { "LOVED": true, "MEH": false },
        }))
        .expect("a solids row");
        assert_eq!(row.reaction(), Some(SolidsReaction::Loved));
    }

    #[test]
    fn a_mode_this_crate_does_not_know_fails_to_decode_rather_than_decoding_wrong() {
        let unknown = serde_json::from_value::<FeedInterval>(json!({
            "mode": "intravenous", "start": 1,
        }));
        assert!(unknown.is_err());
    }

    #[test]
    fn the_mode_is_written_back_when_a_row_is_serialized() {
        let row = FeedInterval::Bottle(BottleFeedInterval {
            start: Number::Float(1.0),
            last_updated: None,
            bottle_type: BottleType::Formula,
            amount: Number::Float(90.0),
            units: VolumeUnits::Millilitres,
            offset: Number::Float(-240.0),
            end_offset: None,
            notes: None,
        });
        let value = serde_json::to_value(&row).expect("serializing");
        assert_eq!(value["mode"], json!("bottle"));
        assert_eq!(value["bottleType"], json!("Formula"));
    }
}
