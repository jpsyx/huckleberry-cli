//! The pieces every Huckleberry document is built from.

use serde::{Deserialize, Serialize};

/// A Huckleberry number, which may be stored as an integer or as a double.
///
/// The app is not consistent about this, and it matters: `offset` comes back
/// as `-240` from one write and `-240.0` from another, and a value written
/// back in the other shape is a value the app may not read the same way. This
/// type keeps whichever one arrived.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Number {
    /// A whole number, stored as a Firestore `integerValue`.
    Integer(i64),
    /// A fractional number, stored as a Firestore `doubleValue`.
    Float(f64),
}

impl Number {
    /// The value as a float, which is what arithmetic wants.
    #[must_use]
    pub const fn as_f64(self) -> f64 {
        match self {
            Self::Integer(value) => value as f64,
            Self::Float(value) => value,
        }
    }

    /// The value truncated towards zero, which is what a Unix timestamp in
    /// seconds wants.
    #[must_use]
    pub const fn as_i64(self) -> i64 {
        match self {
            Self::Integer(value) => value,
            Self::Float(value) => value as i64,
        }
    }
}

impl From<i64> for Number {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<f64> for Number {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl core::fmt::Display for Number {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Integer(value) => write!(formatter, "{value}"),
            Self::Float(value) => write!(formatter, "{value}"),
        }
    }
}

/// The timestamp shape Huckleberry writes.
///
/// Not a Firestore timestamp: the app stores a plain map with a `seconds`
/// field, and writing a real `timestampValue` in its place would be a
/// different type in the same slot.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Timestamp {
    /// Seconds since the Unix epoch.
    pub seconds: Number,
    /// The sub-second part, when the app bothered to write one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nanos: Option<i64>,
}

impl Timestamp {
    /// A timestamp at a moment, in the float shape the app writes.
    #[must_use]
    pub const fn at(seconds: f64) -> Self {
        Self {
            seconds: Number::Float(seconds),
            nanos: None,
        }
    }
}

/// A value Huckleberry stores as either text or a number.
///
/// `birthdate`, `nightStart` and `morningCutoff` on a child profile are all
/// like this: the app has written both shapes over the years and reads either.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TextOrNumber {
    /// Stored as a number.
    Number(Number),
    /// Stored as text.
    Text(String),
}

impl TextOrNumber {
    /// The value as a float, parsing the text form. `None` when the text is
    /// not a number, which is what a date like `2025-09-01` is.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Number(number) => Some(number.as_f64()),
            Self::Text(text) => text.trim().parse().ok(),
        }
    }

    /// The value as text, whichever shape it was stored in.
    #[must_use]
    pub fn to_text(&self) -> String {
        match self {
            Self::Number(number) => number.to_string(),
            Self::Text(text) => text.clone(),
        }
    }
}

/// The batched-rows wrapper Huckleberry writes when a subcollection gets long.
///
/// Rather than one document per row for ever, older rows are packed into a
/// single document with `multi: true` and the rows themselves under `data`.
/// The nested rows cannot be filtered by a query, because Firestore indexes
/// fields and not map entries, so every range read in this crate is two reads:
/// the loose rows, and then these.
///
/// The generic parameter is the row type. Reads instantiate it with
/// `serde_json::Value` so that one unfamiliar row can be skipped instead of
/// costing the caller the whole batch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
// `#[serde(default)]` on `data` otherwise makes the derive demand `T: Default`,
// which a row type has no reason to be.
#[serde(bound(serialize = "T: Serialize", deserialize = "T: Deserialize<'de>"))]
pub struct MultiContainer<T> {
    /// Always true; this is the marker the query filters on.
    pub multi: bool,
    /// Whether the app will pack more rows into this document.
    #[serde(
        rename = "hasMoreRoom",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub has_more_room: Option<bool>,
    /// When the batch last changed.
    #[serde(
        rename = "lastUpdated",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub last_updated: Option<Number>,
    /// The rows, keyed by their own identifiers.
    #[serde(default)]
    pub data: std::collections::BTreeMap<String, T>,
}

string_enum! {
    /// A day of the week, as a reminder schedule names it.
    ReminderDay {
        Sunday => "Sun",
        Monday => "Mon",
        Tuesday => "Tue",
        Wednesday => "Wed",
        Thursday => "Thu",
        Friday => "Fri",
        Saturday => "Sat",
    }
}

/// The "remind me in N" half of a reminder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReminderIn {
    /// How long, in the unit the app decides.
    pub value: Number,
    /// Whether the reminder is suppressed overnight.
    #[serde(rename = "daytimeOnly")]
    pub daytime_only: bool,
    /// Whether it fires at all.
    pub enabled: bool,
    /// Whether it makes a sound.
    pub sound: bool,
    /// Whether it vibrates.
    pub vibration: bool,
    /// Which days it applies to.
    pub days: Vec<ReminderDay>,
}

/// One entry in the "remind me at" half of a reminder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AtReminderEntry {
    /// The time of day.
    pub value: Number,
    /// Whether it fires at all.
    pub enabled: bool,
    /// Whether it makes a sound.
    pub sound: bool,
    /// Whether it vibrates.
    pub vibration: bool,
    /// Which days it applies to.
    pub days: Vec<ReminderDay>,
}

string_enum! {
    /// Which half of a reminder is in force.
    ReminderMode {
        At => "at",
        In => "in",
    }
}

/// A reminder, as the feed, diaper and health trackers store one.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ReminderV2 {
    /// Fixed times of day.
    #[serde(
        rename = "atReminder",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub at_reminder: Option<std::collections::BTreeMap<String, AtReminderEntry>>,
    /// An interval since the last event.
    #[serde(
        rename = "inReminder",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub in_reminder: Option<ReminderIn>,
    /// Which of the two is in use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<ReminderMode>,
}

#[cfg(test)]
mod numbers {
    use super::*;

    #[test]
    fn an_integer_stays_an_integer_through_a_round_trip() {
        let text = serde_json::to_string(&Number::Integer(-240)).expect("serializing");
        assert_eq!(text, "-240");
        assert_eq!(
            serde_json::from_str::<Number>(&text).expect("parsing"),
            Number::Integer(-240)
        );
    }

    #[test]
    fn a_whole_float_stays_a_float_through_a_round_trip() {
        let text = serde_json::to_string(&Number::Float(-240.0)).expect("serializing");
        assert_eq!(text, "-240.0");
        assert_eq!(
            serde_json::from_str::<Number>(&text).expect("parsing"),
            Number::Float(-240.0)
        );
    }

    #[test]
    fn both_shapes_read_as_the_same_quantity() {
        assert!(
            (Number::Integer(3600).as_f64() - Number::Float(3600.0).as_f64()).abs() < f64::EPSILON
        );
        assert_eq!(Number::Float(3600.9).as_i64(), 3600);
    }
}

#[cfg(test)]
mod string_enums {
    use super::*;
    use core::str::FromStr;

    #[test]
    fn a_known_spelling_round_trips() {
        assert_eq!(ReminderDay::from_wire("Mon"), ReminderDay::Monday);
        assert_eq!(ReminderDay::Monday.as_str(), "Mon");
    }

    #[test]
    fn an_unknown_spelling_is_carried_through_rather_than_failing() {
        let unknown = ReminderDay::from_wire("Caturday");
        assert_eq!(unknown, ReminderDay::Unknown("Caturday".to_owned()));
        assert_eq!(unknown.as_str(), "Caturday");
        assert!(!unknown.is_known());
    }

    #[test]
    fn a_typed_value_is_rejected_rather_than_stored_as_a_new_one() {
        assert!(ReminderDay::from_str("Mondayy").is_err());
        assert_eq!(
            ReminderDay::from_str("Tue").expect("a real day"),
            ReminderDay::Tuesday
        );
    }

    #[test]
    fn an_unknown_value_survives_serialization_unchanged() {
        let unknown = ReminderDay::Unknown("Caturday".to_owned());
        let text = serde_json::to_string(&unknown).expect("serializing");
        assert_eq!(text, "\"Caturday\"");
        assert_eq!(
            serde_json::from_str::<ReminderDay>(&text).expect("parsing"),
            unknown
        );
    }

    #[test]
    fn the_names_are_offered_in_declaration_order() {
        assert_eq!(ReminderDay::NAMES[0], "Sun");
        assert_eq!(ReminderDay::NAMES.len(), 7);
    }
}

#[cfg(test)]
mod timestamps {
    use super::*;

    #[test]
    fn a_timestamp_without_nanos_writes_only_seconds() {
        let text = serde_json::to_string(&Timestamp::at(1_758_572_400.0)).expect("serializing");
        assert_eq!(text, r#"{"seconds":1758572400.0}"#);
    }
}
