//! `feed/{cid}`: nursing, bottles and solids all live in one tracker.
//!
//! Split four ways: [`units`] is the words a feed is described with,
//! [`timer`] is the nursing session in progress, [`prefs`] is the last feed of
//! each kind, and [`interval`] is the history.
//!
//! Three things in this tracker routinely surprise people:
//!
//! - **The feed timer's `timerStartTime` is in seconds**, unlike the sleep
//!   timer's milliseconds, and it is not the start of the feed. It is the
//!   start of the *current side*, and it resets on every switch and resume.
//!   `feedStartTime` is the start of the feed.
//! - **`lastSide` can be the literal string `"none"`** while a switch or a
//!   resume is in flight. It is a transition marker, not "no side".
//! - **A bottle row and the `prefs.lastBottle` summary name the same two
//!   numbers differently**: the row says `amount`/`units`, the summary says
//!   `bottleAmount`/`bottleUnits`.

pub mod interval;
pub mod prefs;
pub mod timer;
pub mod units;

pub use interval::{
    BottleFeedInterval, BreastFeedInterval, FeedInterval, FeedMultiContainer, SolidsFeedInterval,
};
pub use prefs::{FeedPrefs, LastBottle, LastNursing, LastSide, LastSolid};
pub use timer::{FeedDocument, FeedTimer};
pub use units::{BottleType, FeedSide, MILLILITRES_PER_OUNCE, VolumeUnits};
