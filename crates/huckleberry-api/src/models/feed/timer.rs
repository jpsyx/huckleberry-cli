//! `feed/{cid}.timer`: the nursing session in progress, if there is one.
//!
//! The timer banks seconds per side. `leftDuration` and `rightDuration` hold
//! what is already finished, and the side running now is measured from
//! `timerStartTime`, which **resets on every switch and every resume** and is
//! in **seconds**, unlike the sleep timer's milliseconds. `feedStartTime` is
//! the start of the feed itself.

use serde::{Deserialize, Serialize};

use super::prefs::FeedPrefs;
use super::units::FeedSide;
use crate::models::common::{Number, Timestamp};

/// `feed/{cid}.timer`: the nursing session in progress, if there is one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedTimer {
    /// Whether a session is running.
    pub active: bool,
    /// Whether it is paused.
    pub paused: bool,
    /// When the document last changed.
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
    /// When the whole feed started, in seconds.
    #[serde(
        rename = "feedStartTime",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub feed_start_time: Option<Number>,
    /// When the current side started, in seconds. Resets on switch and resume.
    #[serde(
        rename = "timerStartTime",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub timer_start_time: Option<Number>,
    /// This session's identifier.
    pub uuid: String,
    /// Seconds banked on the left so far.
    #[serde(
        rename = "leftDuration",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub left_duration: Option<Number>,
    /// Seconds banked on the right so far.
    #[serde(
        rename = "rightDuration",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub right_duration: Option<Number>,
    /// The side last fed on, or `none` mid-transition.
    #[serde(
        rename = "lastSide",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub last_side: Option<FeedSide>,
    /// The side being fed on now.
    #[serde(
        rename = "activeSide",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
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
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub timer: Option<FeedTimer>,
    /// The last feed of each kind, and the tracker's settings.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
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
