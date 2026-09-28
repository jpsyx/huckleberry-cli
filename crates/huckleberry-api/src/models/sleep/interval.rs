//! Sleep history, and the summary kept beside the timer.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::details::SleepDetails;
use super::timer::SleepTimer;
use crate::models::common::{MultiContainer, Number, Timestamp};

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
pub type SleepMultiContainer = MultiContainer<SleepInterval>;

/// Sleep rows keyed by id, as a batch stores them.
pub type SleepRows = BTreeMap<String, SleepInterval>;

#[cfg(test)]
mod intervals {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_document_with_an_idle_timer_reports_no_running_sleep() {
        let document: SleepDocument = serde_json::from_value(json!({
            "timer": { "active": false, "paused": false, "uuid": "0123456789abcdef" },
        }))
        .expect("a sleep document");
        assert!(document.running_timer().is_none());
    }

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
