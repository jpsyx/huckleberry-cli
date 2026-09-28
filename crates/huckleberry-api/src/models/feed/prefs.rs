//! `feed/{cid}.prefs`: the last feed of each kind, and the tracker's defaults.
//!
//! One naming trap lives here. A bottle **row** spells its two numbers
//! `amount` and `units`; the `lastBottle` **summary** spells the same two
//! `bottleAmount` and `bottleUnits`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::units::{BottleType, FeedSide, VolumeUnits};
use crate::models::common::{Number, ReminderV2, Timestamp};
use crate::models::solids::SolidsFoodEntry;

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
