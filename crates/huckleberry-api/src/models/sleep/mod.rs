//! `sleep/{cid}`: the running timer, the last sleep, and the history.
//!
//! Split three ways: [`details`] is what was recorded about a sleep,
//! [`timer`] is the one in progress, and [`interval`] is the history and the
//! summary that sits beside the timer.
//!
//! One unit rule governs the whole tracker and is the single easiest thing to
//! get wrong in this API: **the sleep timer's `timerStartTime` is in
//! milliseconds**, while the feed timer's is in seconds. A sleep start read as
//! seconds lands in 1970; written as seconds it tells the app the baby has
//! been asleep since the Nixon administration. [`SleepTimer::started_at`]
//! exists so nothing has to remember.

pub mod details;
pub mod interval;
pub mod timer;

pub use details::{SleepCondition, SleepDetails, SleepLocations};
pub use interval::{
    LastSleep, SleepDocument, SleepInterval, SleepMultiContainer, SleepPrefs, SleepRows,
};
pub use timer::{SleepSwsAnalytics, SleepSwsDataShown, SleepTimer};
