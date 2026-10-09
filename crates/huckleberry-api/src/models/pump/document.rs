//! The pump tracker document and its last-entry summary, ported from Woyken.

use serde::{Deserialize, Serialize};

use super::{PumpEntryMode, PumpTimer};
use crate::models::common::{Number, ReminderV2, Timestamp};
use crate::models::feed::VolumeUnits;

/// The most recent pumping session, copied into `prefs.lastPump`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LastPump {
    /// Session start, in Unix seconds.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub start: Option<Number>,
    /// Session length, in seconds, when recorded.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub duration: Option<Number>,
    /// Whether amounts were entered separately or as a total.
    #[serde(
        rename = "entryMode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub entry_mode: Option<PumpEntryMode>,
    /// Left amount, or half the combined total.
    #[serde(
        rename = "leftAmount",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub left_amount: Option<Number>,
    /// Right amount, or half the combined total.
    #[serde(
        rename = "rightAmount",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub right_amount: Option<Number>,
    /// The recorded volume units.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub units: Option<VolumeUnits>,
    /// Minutes to add to local time to reach UTC.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub offset: Option<Number>,
}

/// Pump tracker preferences, summary and reminder settings.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PumpPrefs {
    /// The latest recorded pumping session.
    #[serde(
        rename = "lastPump",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub last_pump: Option<LastPump>,
    /// The app's reminder settings.
    #[serde(
        rename = "reminderV2",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub reminder_v2: Option<ReminderV2>,
    /// When preferences last changed.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub timestamp: Option<Timestamp>,
    /// The same synchronization time as a bare number.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub local_timestamp: Option<Number>,
}

/// `pump/{cid}`: the active timer and latest session.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PumpDocument {
    /// The live timer, including the inactive marker after a session ends.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub timer: Option<PumpTimer>,
    /// Preferences and last-session summary.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub prefs: Option<PumpPrefs>,
}

impl PumpDocument {
    /// An active session, whether running or paused.
    #[must_use]
    pub fn running_timer(&self) -> Option<&PumpTimer> {
        self.timer.as_ref().filter(|timer| timer.active)
    }
}
