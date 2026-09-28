//! The operations, split by tracker.
//!
//! Each submodule is an `impl Huckleberry` block, so the whole surface reads
//! as one type while no file grows past the point of being readable. The
//! arithmetic each operation depends on (how long a sleep was, which side is
//! next, what a paused nursing session has banked) lives in a pure function
//! next to the operation that uses it, and those are what the tests assert.

pub mod diaper;
pub mod feed;
pub mod health;
pub mod reads;
pub mod removal;
pub mod rows;
pub mod sleep;
mod sleep_edit;
pub mod solids;
pub mod watch;

pub(crate) mod timing;

pub use reads::Window;

/// What happened when a timer was asked to change.
///
/// The Python client logs and returns for all three of these, which leaves a
/// caller unable to tell "paused it" from "there was nothing to pause". A
/// command-line tool has to print something different in each case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerChange {
    /// The change was made.
    Applied,
    /// There is no session running, so there was nothing to change.
    NotRunning,
    /// The session was already in the state that was asked for.
    Unchanged,
}

impl TimerChange {
    /// Whether anything was written.
    #[must_use]
    pub const fn changed_anything(self) -> bool {
        matches!(self, Self::Applied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_applied_change_wrote_anything() {
        assert!(TimerChange::Applied.changed_anything());
        assert!(!TimerChange::NotRunning.changed_anything());
        assert!(!TimerChange::Unchanged.changed_anything());
    }
}
