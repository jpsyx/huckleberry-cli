//! `sleep/{cid}.timer`: the sleep in progress, if there is one.
//!
//! The unit rule lives here. **`timerStartTime` is in milliseconds** for
//! sleep, and in seconds for feeding. [`SleepTimer::started_at`] is the only
//! place that divides, so nothing downstream has to remember.

use serde::{Deserialize, Serialize};

use super::details::SleepDetails;
use crate::models::common::{Number, Timestamp};

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
    fn an_inactive_timer_is_a_finished_session_and_not_a_paused_one() {
        let mut timer = running(1.0);
        timer.active = false;
        timer.paused = true;
        assert!(
            timer.elapsed(2.0).is_none(),
            "a finished sleep looks paused and is not one"
        );
    }
}
