//! The event shapes the whole tool speaks.
//!
//! Between the API's Firestore models and anything that prints, there is one
//! normalization step, and this is its output. The models are faithful to what
//! Huckleberry stores, which means optional fields, two spellings for the same
//! number, and units that differ per row. These types have made all of those
//! decisions already: a volume is millilitres, a duration is seconds, a time
//! is a Unix second, and a value that was never recorded is `None` rather than
//! a zero that would read as a measurement.
//!
//! Nothing in `domain` reads the clock. Every function takes `now`, which is
//! what makes the day-boundary and timezone cases testable.

use huckleberry_api::RowRef;
use serde::{Deserialize, Serialize};

/// A Unix timestamp in seconds.
pub type Seconds = f64;

/// One sleep.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SleepEvent {
    /// Where the row lives in Huckleberry, when it came from there rather
    /// than from a snapshot written before this tool could say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<RowRef>,
    /// A stable identifier for this row.
    pub id: String,
    /// When it started.
    pub start: Seconds,
    /// How long it lasted.
    pub duration: Seconds,
    /// Only the location flags recorded true, by their wire names.
    pub locations: Vec<String>,
    /// Only the settling flags recorded true.
    pub start_mood: Vec<String>,
    /// Only the waking flags recorded true.
    pub end_mood: Vec<String>,
    /// Whatever the parent typed.
    pub notes: Option<String>,
}

impl SleepEvent {
    /// When it ended.
    #[must_use]
    pub fn end(&self) -> Seconds {
        self.start + self.duration
    }
}

/// One feed, of whichever kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum FeedEvent {
    /// A bottle.
    Bottle {
        /// Where the row lives in Huckleberry, when it came from there.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<RowRef>,
        /// A stable identifier for this row.
        id: String,
        /// When it was given.
        start: Seconds,
        /// How much, in millilitres. `None` when the row recorded no amount,
        /// which is not the same as zero and must never become one.
        amount_ml: Option<f64>,
        /// What was in it.
        bottle_type: Option<String>,
        /// Whatever the parent typed.
        notes: Option<String>,
    },
    /// A nursing session.
    Nursing {
        /// Where the row lives in Huckleberry, when it came from there.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<RowRef>,
        /// A stable identifier for this row.
        id: String,
        /// When it started.
        start: Seconds,
        /// Seconds on the left.
        left_seconds: f64,
        /// Seconds on the right.
        right_seconds: f64,
        /// Which side it ended on.
        last_side: Option<String>,
        /// Whatever the parent typed.
        notes: Option<String>,
    },
    /// A meal.
    Solids {
        /// Where the row lives in Huckleberry, when it came from there.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<RowRef>,
        /// A stable identifier for this row.
        id: String,
        /// When it was.
        start: Seconds,
        /// What was eaten, by name.
        foods: Vec<String>,
        /// How it went.
        reaction: Option<String>,
        /// Whatever the parent typed.
        notes: Option<String>,
    },
}

impl FeedEvent {
    /// When the feed was.
    #[must_use]
    pub const fn start(&self) -> Seconds {
        match self {
            Self::Bottle { start, .. }
            | Self::Nursing { start, .. }
            | Self::Solids { start, .. } => *start,
        }
    }

    /// Where the row lives in Huckleberry, when it came from there.
    #[must_use]
    pub const fn at(&self) -> Option<&RowRef> {
        match self {
            Self::Bottle { at, .. } | Self::Nursing { at, .. } | Self::Solids { at, .. } => {
                at.as_ref()
            }
        }
    }

    /// A stable identifier for this row.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Bottle { id, .. } | Self::Nursing { id, .. } | Self::Solids { id, .. } => id,
        }
    }

    /// Whatever the parent typed.
    #[must_use]
    pub fn notes(&self) -> Option<&str> {
        match self {
            Self::Bottle { notes, .. }
            | Self::Nursing { notes, .. }
            | Self::Solids { notes, .. } => notes.as_deref(),
        }
    }

    /// Millilitres taken, for a bottle.
    #[must_use]
    pub const fn millilitres(&self) -> Option<f64> {
        match self {
            Self::Bottle { amount_ml, .. } => *amount_ml,
            _ => None,
        }
    }

    /// Seconds spent nursing, both sides together.
    #[must_use]
    pub const fn nursing_seconds(&self) -> Option<f64> {
        match self {
            Self::Nursing {
                left_seconds,
                right_seconds,
                ..
            } => Some(*left_seconds + *right_seconds),
            _ => None,
        }
    }

    /// Whether this feed is milk at all, which is what a feed count means.
    #[must_use]
    pub const fn is_milk(&self) -> bool {
        matches!(self, Self::Bottle { .. } | Self::Nursing { .. })
    }
}

/// How much, as the app's three buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Size {
    /// The app's "little".
    Small,
    /// The app's "medium".
    Medium,
    /// The app's "big".
    Large,
}

/// One diaper, or one potty trip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiaperEvent {
    /// Where the row lives in Huckleberry, when it came from there rather
    /// than from a snapshot written before this tool could say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<RowRef>,
    /// A stable identifier for this row.
    pub id: String,
    /// When it was.
    pub start: Seconds,
    /// What was in it, as the app's own word.
    pub mode: String,
    /// Derived from the mode, which is the source of truth.
    pub wet: bool,
    /// Derived from the mode.
    pub dirty: bool,
    /// How much wet.
    pub pee_size: Option<Size>,
    /// How much dirty.
    pub poo_size: Option<Size>,
    /// The colour.
    pub color: Option<String>,
    /// The consistency.
    pub consistency: Option<String>,
    /// Whether a rash was noted.
    pub rash: bool,
    /// Whether this was a potty trip rather than a diaper.
    pub potty: bool,
    /// Whatever the parent typed.
    pub notes: Option<String>,
}

/// One pumping session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PumpEvent {
    /// Where the row lives in Huckleberry, when it came from there rather
    /// than from a snapshot written before this tool could say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<RowRef>,
    /// A stable identifier for this row.
    pub id: String,
    /// When it was.
    pub start: Seconds,
    /// Millilitres from the left.
    pub left_ml: Option<f64>,
    /// Millilitres from the right.
    pub right_ml: Option<f64>,
    /// Both together. `None` when neither side was recorded.
    pub total_ml: Option<f64>,
    /// How long it took.
    pub duration_seconds: Option<f64>,
    /// Whatever the parent typed.
    pub notes: Option<String>,
}

/// One milestone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MilestoneEvent {
    /// Where the row lives in Huckleberry, when it came from there rather
    /// than from a snapshot written before this tool could say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<RowRef>,
    /// A stable identifier for this row.
    pub id: String,
    /// When it happened.
    pub start: Seconds,
    /// What it was.
    pub name: String,
    /// Which group it belongs to.
    pub category: Option<String>,
    /// Whatever the parent typed.
    pub notes: Option<String>,
    /// Whether a photo was attached. The photo itself needs a download token
    /// the data does not carry, so this is a record that one exists.
    pub has_photo: bool,
}

/// One growth measurement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrowthPoint {
    /// When it was taken.
    pub start: Seconds,
    /// Kilograms, when the row said which unit it was in.
    pub weight_kg: Option<f64>,
    /// As recorded, in whatever unit.
    pub weight_raw: Option<f64>,
    /// The unit it was recorded in.
    pub weight_units: Option<String>,
}

/// What is happening right now, from the trackers' live timers.
///
/// `#[serde(default)]` so a snapshot written before a field existed still
/// reads: an absent flag means nothing is running, which is the truth.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LiveState {
    /// Whether a sleep is running.
    pub sleep_active: bool,
    /// When it started, converted from the timer's milliseconds.
    pub sleep_start: Option<Seconds>,
    /// Whether that sleep is paused.
    pub sleep_paused: bool,
    /// Whether a nursing session is running.
    pub nursing_active: bool,
    /// When it started. The feed timer's value is already seconds.
    pub nursing_start: Option<Seconds>,
    /// Which side.
    pub nursing_side: Option<String>,
    /// Whether that session is paused.
    pub nursing_paused: bool,
}

/// The child every number here is about.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Child {
    /// The child id.
    pub cid: String,
    /// What to call them.
    pub name: String,
    /// Date of birth, as text, when the profile has one.
    pub birthdate: Option<String>,
    /// When night begins, on a 24-hour clock: `20.0` is 8pm.
    pub night_start_hour: f64,
    /// When morning begins: `6.75` is 6:45am.
    pub morning_cutoff_hour: f64,
}

/// Something that could not be read, and why.
///
/// Several Huckleberry trackers are simply not readable on some accounts.
/// Recording the refusal beside the data is better than a silently short
/// dataset: a parent seeing no pumping sessions should be able to tell "none
/// logged" from "not allowed to look".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollectionNote {
    /// Which collection.
    pub collection: String,
    /// What went wrong, as a person would read it.
    pub problem: String,
}

/// Everything the tool knows about one child over one window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dataset {
    /// When this was pulled.
    pub fetched_at: Seconds,
    /// The family's timezone, which every day boundary is computed in.
    pub timezone: String,
    /// How many days of history this covers.
    pub days: i64,
    /// The child.
    pub child: Child,
    /// The last growth measurement.
    pub growth: Option<GrowthPoint>,
    /// Sleeps, oldest first.
    pub sleep: Vec<SleepEvent>,
    /// Feeds, oldest first.
    pub feeds: Vec<FeedEvent>,
    /// Diapers and potty trips, oldest first.
    pub diapers: Vec<DiaperEvent>,
    /// Pumping sessions, oldest first.
    pub pumps: Vec<PumpEvent>,
    /// Milestones, oldest first.
    pub milestones: Vec<MilestoneEvent>,
    /// What the trackers say is happening now.
    pub live: LiveState,
    /// Anything that could not be read.
    pub notes: Vec<CollectionNote>,
}

impl Dataset {
    /// The most recent feed, if there is one.
    ///
    /// Found by comparing timestamps rather than by taking the last element.
    /// `normalize` does sort, but a snapshot read off disk was written by
    /// somebody else's version of this tool or edited by hand, and "when did
    /// he last eat" is the wrong question to answer from an assumption.
    #[must_use]
    pub fn last_feed(&self) -> Option<&FeedEvent> {
        newest(&self.feeds, FeedEvent::start)
    }

    /// The most recent diaper, if there is one.
    #[must_use]
    pub fn last_diaper(&self) -> Option<&DiaperEvent> {
        newest(&self.diapers, |diaper| diaper.start)
    }

    /// The most recent sleep, by when it ended: a long sleep that began
    /// before a short one can still be the one that finished last, and "how
    /// long has he been awake" is measured from whichever that was.
    #[must_use]
    pub fn last_sleep(&self) -> Option<&SleepEvent> {
        newest(&self.sleep, SleepEvent::end)
    }

    /// Whether anything at all was logged in the window.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sleep.is_empty()
            && self.feeds.is_empty()
            && self.diapers.is_empty()
            && self.pumps.is_empty()
            && self.milestones.is_empty()
    }
}

/// The item with the greatest timestamp, or `None` when there are none.
fn newest<T>(items: &[T], at: impl Fn(&T) -> Seconds) -> Option<&T> {
    items.iter().max_by(|left, right| {
        at(left)
            .partial_cmp(&at(right))
            .unwrap_or(core::cmp::Ordering::Equal)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bottle(amount: Option<f64>) -> FeedEvent {
        FeedEvent::Bottle {
            at: None,
            id: "b1".to_owned(),
            start: 100.0,
            amount_ml: amount,
            bottle_type: Some("Formula".to_owned()),
            notes: None,
        }
    }

    #[test]
    fn a_bottle_with_no_amount_has_no_volume_rather_than_zero() {
        assert_eq!(bottle(None).millilitres(), None);
        assert_eq!(bottle(Some(90.0)).millilitres(), Some(90.0));
    }

    #[test]
    fn a_nursing_session_adds_its_two_sides() {
        let nursing = FeedEvent::Nursing {
            at: None,
            id: "n1".to_owned(),
            start: 100.0,
            left_seconds: 300.0,
            right_seconds: 120.0,
            last_side: Some("right".to_owned()),
            notes: None,
        };
        assert_eq!(nursing.nursing_seconds(), Some(420.0));
        assert_eq!(nursing.millilitres(), None);
    }

    #[test]
    fn solids_are_a_feed_but_not_a_milk_feed() {
        let meal = FeedEvent::Solids {
            at: None,
            id: "s1".to_owned(),
            start: 100.0,
            foods: vec!["Avocado".to_owned()],
            reaction: Some("LOVED".to_owned()),
            notes: None,
        };
        assert!(!meal.is_milk());
        assert!(bottle(None).is_milk());
    }

    #[test]
    fn a_sleep_knows_when_it_ended() {
        let sleep = SleepEvent {
            at: None,
            id: "s".to_owned(),
            start: 1_000.0,
            duration: 3_600.0,
            locations: Vec::new(),
            start_mood: Vec::new(),
            end_mood: Vec::new(),
            notes: None,
        };
        assert!((sleep.end() - 4_600.0).abs() < f64::EPSILON);
    }

    #[test]
    fn the_last_feed_is_the_newest_one_even_out_of_order() {
        let dataset = Dataset {
            fetched_at: 0.0,
            timezone: "UTC".to_owned(),
            days: 1,
            child: Child {
                cid: "c".to_owned(),
                name: "B".to_owned(),
                birthdate: None,
                night_start_hour: 20.0,
                morning_cutoff_hour: 7.0,
            },
            growth: None,
            sleep: vec![
                SleepEvent {
                    at: None,
                    id: "late-start".to_owned(),
                    start: 500.0,
                    duration: 10.0,
                    locations: Vec::new(),
                    start_mood: Vec::new(),
                    end_mood: Vec::new(),
                    notes: None,
                },
                SleepEvent {
                    at: None,
                    id: "early-start-later-end".to_owned(),
                    start: 100.0,
                    duration: 1_000.0,
                    locations: Vec::new(),
                    start_mood: Vec::new(),
                    end_mood: Vec::new(),
                    notes: None,
                },
            ],
            // Deliberately newest first, which is the order a hand-written
            // snapshot is most likely to arrive in.
            feeds: vec![
                bottle(Some(90.0)),
                FeedEvent::Bottle {
                    at: None,
                    id: "older".to_owned(),
                    start: 1.0,
                    amount_ml: Some(10.0),
                    bottle_type: None,
                    notes: None,
                },
            ],
            diapers: Vec::new(),
            pumps: Vec::new(),
            milestones: Vec::new(),
            live: LiveState::default(),
            notes: Vec::new(),
        };
        assert_eq!(dataset.last_feed().expect("a feed").id(), "b1");
        assert_eq!(
            dataset.last_sleep().expect("a sleep").id,
            "early-start-later-end",
            "the sleep that finished last is the one to measure awake time from"
        );
    }

    #[test]
    fn a_feed_event_survives_a_round_trip_through_json() {
        let feed = bottle(Some(90.0));
        let text = serde_json::to_string(&feed).expect("serializing");
        assert_eq!(
            serde_json::from_str::<FeedEvent>(&text).expect("parsing"),
            feed
        );
    }
}
