//! `diaper/{cid}`: nappies and potty trips, in one tracker.
//!
//! These are instant events, not timed sessions, so there is no timer on the
//! document: only the history and a summary of the last one.

use serde::{Deserialize, Serialize};

use super::common::{Number, ReminderV2, Timestamp};

string_enum! {
    /// What was in it.
    DiaperMode {
        Pee => "pee",
        Poo => "poo",
        Both => "both",
        Dry => "dry",
    }
}

impl DiaperMode {
    /// Whether this counts as a wet nappy.
    #[must_use]
    pub const fn is_wet(&self) -> bool {
        matches!(self, Self::Pee | Self::Both)
    }

    /// Whether this counts as a dirty nappy.
    #[must_use]
    pub const fn is_dirty(&self) -> bool {
        matches!(self, Self::Poo | Self::Both)
    }
}

string_enum! {
    /// The colour, as the app offers it.
    PooColor {
        Yellow => "yellow",
        Brown => "brown",
        Black => "black",
        Green => "green",
        Red => "red",
        Gray => "gray",
    }
}

string_enum! {
    /// The consistency, as the app offers it.
    PooConsistency {
        Solid => "solid",
        Loose => "loose",
        Runny => "runny",
        Mucousy => "mucousy",
        Hard => "hard",
        Pebbles => "pebbles",
        Diarrhea => "diarrhea",
    }
}

string_enum! {
    /// How a potty trip went.
    PottyResult {
        SatButDry => "satButDry",
        WentPotty => "wentPotty",
        Accident => "accident",
    }
}

string_enum! {
    /// How much, as the app's three buttons.
    DiaperAmount {
        Little => "little",
        Medium => "medium",
        Big => "big",
    }
}

impl DiaperAmount {
    /// The number the app stores for this button.
    ///
    /// Not a scale: the app writes exactly 0, 50 or 100, and a value that is
    /// none of the three is treated as no quantity recorded at all.
    #[must_use]
    pub const fn to_stored(&self) -> Option<f64> {
        match self {
            Self::Little => Some(0.0),
            Self::Medium => Some(50.0),
            Self::Big => Some(100.0),
            Self::Unknown(_) => None,
        }
    }

    /// Reads one of the three stored numbers back.
    #[must_use]
    pub fn from_stored(value: f64) -> Option<Self> {
        if value <= 0.0 {
            Some(Self::Little)
        } else if value <= 50.0 {
            Some(Self::Medium)
        } else {
            Some(Self::Big)
        }
    }
}

/// How much of each, on one nappy.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DiaperQuantity {
    /// How much wet.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub pee: Option<Number>,
    /// How much dirty.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub poo: Option<Number>,
}

impl DiaperQuantity {
    /// Whether anything at all was recorded, which is what decides whether the
    /// field is written to Firestore.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.pee.is_none() && self.poo.is_none()
    }
}

/// One row of `diaper/{cid}/intervals`. Also a potty trip, which the app
/// stores here with `isPotty` set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiaperEntry {
    /// What was in it.
    pub mode: DiaperMode,
    /// When it was, in seconds.
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
    /// How much of each.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub quantity: Option<DiaperQuantity>,
    /// The colour.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub color: Option<PooColor>,
    /// The consistency.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub consistency: Option<PooConsistency>,
    /// Whether a rash was noted.
    #[serde(
        rename = "diaperRash",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub diaper_rash: Option<bool>,
    /// Whatever the parent typed.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub notes: Option<String>,
    /// Whether this was a potty trip rather than a nappy.
    #[serde(
        rename = "isPotty",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub is_potty: Option<bool>,
    /// How the potty trip went.
    #[serde(
        rename = "howItHappened",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub how_it_happened: Option<PottyResult>,
}

impl DiaperEntry {
    /// Whether this row is a potty trip.
    #[must_use]
    pub fn is_potty_trip(&self) -> bool {
        self.is_potty == Some(true)
    }

    /// Whether a rash was noted.
    #[must_use]
    pub fn has_rash(&self) -> bool {
        self.diaper_rash == Some(true)
    }
}

/// The summary of the last nappy.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LastDiaper {
    /// When it was, in seconds.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub start: Option<Number>,
    /// What was in it.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub mode: Option<DiaperMode>,
    /// The timezone offset, in minutes.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub offset: Option<Number>,
}

/// The summary of the last potty trip.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LastPotty {
    /// What happened.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub mode: Option<DiaperMode>,
    /// When it was, in seconds.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub start: Option<Number>,
    /// The timezone offset, in minutes.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub offset: Option<Number>,
}

/// `diaper/{cid}.prefs`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DiaperPrefs {
    /// The last nappy.
    #[serde(
        rename = "lastDiaper",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub last_diaper: Option<LastDiaper>,
    /// The last potty trip.
    #[serde(
        rename = "lastPotty",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub last_potty: Option<LastPotty>,
    /// The nappy reminder.
    #[serde(
        rename = "reminderV2",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub reminder: Option<ReminderV2>,
    /// When the preferences last changed.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub timestamp: Option<Timestamp>,
    /// The same moment, as a bare number.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub local_timestamp: Option<Number>,
}

/// `diaper/{cid}`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DiaperDocument {
    /// The last nappy, the last potty trip, and the tracker's settings.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub prefs: Option<DiaperPrefs>,
}

/// A batch of nappy rows.
pub type DiaperMultiContainer = super::common::MultiContainer<DiaperEntry>;

#[cfg(test)]
mod modes {
    use super::*;

    #[test]
    fn both_counts_as_wet_and_dirty() {
        assert!(DiaperMode::Both.is_wet() && DiaperMode::Both.is_dirty());
    }

    #[test]
    fn dry_counts_as_neither() {
        assert!(!DiaperMode::Dry.is_wet() && !DiaperMode::Dry.is_dirty());
    }

    #[test]
    fn a_mode_this_crate_does_not_know_counts_as_neither() {
        let unknown = DiaperMode::Unknown("mixed".to_owned());
        assert!(!unknown.is_wet() && !unknown.is_dirty());
    }
}

#[cfg(test)]
mod amounts {
    use super::*;

    #[test]
    fn the_three_buttons_store_the_three_numbers_the_app_writes() {
        assert_eq!(DiaperAmount::Little.to_stored(), Some(0.0));
        assert_eq!(DiaperAmount::Medium.to_stored(), Some(50.0));
        assert_eq!(DiaperAmount::Big.to_stored(), Some(100.0));
    }

    #[test]
    fn the_stored_numbers_read_back_as_the_buttons() {
        assert_eq!(DiaperAmount::from_stored(0.0), Some(DiaperAmount::Little));
        assert_eq!(DiaperAmount::from_stored(50.0), Some(DiaperAmount::Medium));
        assert_eq!(DiaperAmount::from_stored(100.0), Some(DiaperAmount::Big));
    }

    #[test]
    fn an_amount_this_crate_does_not_know_stores_nothing_rather_than_a_guess() {
        assert_eq!(
            DiaperAmount::Unknown("enormous".to_owned()).to_stored(),
            None
        );
    }
}

#[cfg(test)]
mod rows {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_minimal_row_is_all_the_app_writes_by_default() {
        let row: DiaperEntry = serde_json::from_value(
            json!({ "mode": "pee", "start": 1_758_572_400.0, "offset": -240.0 }),
        )
        .expect("a row");
        assert!(row.mode.is_wet());
        assert!(!row.has_rash());
        assert!(!row.is_potty_trip());
    }

    #[test]
    fn a_potty_trip_is_a_diaper_row_that_says_so() {
        let row: DiaperEntry = serde_json::from_value(json!({
            "mode": "poo", "start": 1.0, "offset": 0.0,
            "isPotty": true, "howItHappened": "wentPotty",
        }))
        .expect("a row");
        assert!(row.is_potty_trip());
        assert_eq!(row.how_it_happened, Some(PottyResult::WentPotty));
    }

    #[test]
    fn a_row_with_nothing_optional_serializes_without_the_optional_keys() {
        let row = DiaperEntry {
            mode: DiaperMode::Dry,
            start: Number::Float(1.0),
            last_updated: None,
            offset: Number::Float(0.0),
            quantity: None,
            color: None,
            consistency: None,
            diaper_rash: None,
            notes: None,
            is_potty: None,
            how_it_happened: None,
        };
        let value = serde_json::to_value(&row).expect("serializing");
        assert_eq!(value, json!({ "mode": "dry", "start": 1.0, "offset": 0.0 }));
    }

    #[test]
    fn an_empty_quantity_knows_it_is_empty() {
        assert!(DiaperQuantity::default().is_empty());
        assert!(
            !DiaperQuantity {
                pee: Some(Number::Float(0.0)),
                poo: None
            }
            .is_empty()
        );
    }
}
