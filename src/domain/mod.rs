//! The analysis layer: everything the tool works out, and nothing it prints.
//!
//! This is the port of the dashboard's `domain/` directory, and it keeps that
//! project's one rule: **nothing here reads the clock, touches the network, or
//! writes to a terminal.** Every function takes `now` as an argument and
//! returns values. That is what makes the awkward cases (a sleep across
//! midnight, a night window that spans midnight, a 23-hour day) testable
//! rather than something you find out about in March.
//!
//! | Module | Answers |
//! | --- | --- |
//! | [`time`] | which local day an instant is on, and how long that day is |
//! | [`clock`] | a time somebody typed, and the instant it means |
//! | [`types`] | the event shapes everything downstream speaks |
//! | [`normalize`] | the API's models turned into those shapes, once |
//! | [`now`] | the four facts on the 3am screen |
//! | [`summaries`] | one row per day, the numbers a pediatrician asks for |
//! | [`stripes`] | the geometry of the 24-hour chart |
//! | [`reference`] | age-aware typical ranges, stated and never prescribed |
//! | [`log`] | one merged, newest-first stream of everything |

pub mod clock;
#[cfg(test)]
pub(crate) mod fixtures;
pub mod log;
pub mod normalize;
pub mod now;
pub mod reference;
pub mod stripes;
pub mod summaries;
pub mod time;
pub mod types;

pub use time::Calendar;
pub use types::Dataset;
