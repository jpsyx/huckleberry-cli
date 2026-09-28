//! `feed/{cid}/intervals`: nursing, bottles and meals, in one collection.
//!
//! `mode` is the discriminator, which is how the app itself tells them apart,
//! and writing a row through [`FeedInterval`] rather than as a bare struct is
//! what puts it on the document.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::units::{BottleType, FeedSide, VolumeUnits};
use crate::models::common::{MultiContainer, Number};
use crate::models::solids::{SolidsFoodEntry, SolidsReaction};

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
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
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
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub end_offset: Option<Number>,
    /// Whatever the parent typed.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
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
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
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
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub end_offset: Option<Number>,
    /// Whatever the parent typed.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
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
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub last_updated: Option<Number>,
    /// The timezone offset, in minutes.
    pub offset: Number,
    /// What was eaten, keyed by food id.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub foods: Option<BTreeMap<String, SolidsFoodEntry>>,
    /// How it went.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub reactions: Option<BTreeMap<String, bool>>,
    /// Whatever the parent typed.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub notes: Option<String>,
    /// A Firebase Storage filename for a photo of the meal.
    #[serde(
        rename = "foodNoteImage",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub food_note_image: Option<String>,
    /// The batch this row came out of, when it came out of one.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub multientry_key: Option<String>,
    /// The timezone offset at the end, in minutes.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
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
pub type FeedMultiContainer = MultiContainer<FeedInterval>;

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
