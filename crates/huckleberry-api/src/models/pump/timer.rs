//! Pump timer fields use milliseconds, as verified by Woyken's upstream port.

use serde::{Deserialize, Serialize};

use super::PumpEntryMode;
use crate::models::common::{Number, Timestamp};
use crate::models::feed::VolumeUnits;

/// `pump/{cid}.timer`, including the stable UUID retained between sessions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PumpTimer {
    /// Whether this timer represents an ongoing session.
    pub active: bool,
    /// Whether the active timer is paused; absent after cancellation/completion.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub paused: Option<bool>,
    /// Synchronization timestamp, independent of the event time.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub timestamp: Option<Timestamp>,
    /// Synchronization timestamp as Unix seconds.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub local_timestamp: Option<Number>,
    /// Session start in milliseconds.
    #[serde(
        rename = "startTime",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub start_time: Option<Number>,
    /// Paused endpoint in milliseconds, removed on resume.
    #[serde(
        rename = "endTime",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub end_time: Option<Number>,
    /// Last selected amount entry mode.
    #[serde(
        rename = "entryMode",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub entry_mode: Option<PumpEntryMode>,
    /// Last selected volume units.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub units: Option<VolumeUnits>,
    /// A note while the timer is running.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub notes: Option<String>,
    /// The identifier observed to persist across timer sessions.
    pub uuid: String,
}

impl PumpTimer {
    /// Session start as Unix seconds rather than Firestore milliseconds.
    #[must_use]
    pub fn started_at(&self) -> Option<f64> {
        self.start_time.map(|time| time.as_f64() / 1000.0)
    }

    /// Elapsed seconds, freezing at the pause time while paused.
    /// Resuming includes the pause in the session, matching the upstream app.
    #[must_use]
    pub fn elapsed_seconds(&self, now: f64) -> Option<f64> {
        if !self.active {
            return None;
        }
        let end = if self.paused == Some(true) {
            self.end_time.map_or(now, |time| time.as_f64() / 1000.0)
        } else {
            now
        };
        let duration = end - self.started_at()?;
        (duration.is_finite() && duration >= 0.0).then_some(duration)
    }
}
