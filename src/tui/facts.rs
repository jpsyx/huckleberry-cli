//! What the shell knows about the child, and how stale it is.
//!
//! Pure: this holds the result of a read and says nothing about how one is
//! made. [`super::data`] does the reading.
//!
//! The rule it exists to keep is the one in
//! [`dashboards.md`](../../docs/dashboards.md): a failed re-read never takes
//! the screen away. The numbers that were there stay there and the failure is
//! recorded beside them, because a screen that blanks itself when the wifi
//! drops is worse than one that admits it is showing something from a minute
//! ago.

use crate::cli::Units;
use crate::domain::Calendar;
use crate::domain::types::Dataset;

/// Everything one read produced, kept together so a screen can never pair a
/// dataset with the calendar or the units from a different one.
pub struct Reading {
    /// What was read.
    pub dataset: Dataset,
    /// The calendar its days are counted in.
    pub calendar: Calendar,
    /// The volume unit the reader chose.
    pub units: Units,
}

/// The shell's copy of the facts, and what is happening to them.
#[derive(Default)]
pub struct Facts {
    reading: Option<Reading>,
    /// Whether a read is in flight.
    pub refreshing: bool,
    /// The last failure, kept so a dropped connection is visible without
    /// taking the numbers away.
    pub trouble: Option<String>,
}

impl Facts {
    /// Nothing read yet.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            reading: None,
            refreshing: false,
            trouble: None,
        }
    }

    /// The last successful read, if there has been one.
    #[must_use]
    pub const fn reading(&self) -> Option<&Reading> {
        self.reading.as_ref()
    }

    /// The child these facts are about, once a read has said.
    #[must_use]
    pub fn child_name(&self) -> Option<&str> {
        self.reading
            .as_ref()
            .map(|reading| reading.dataset.child.name.as_str())
    }

    /// Marks a read as started, so the screen can say so before it finishes.
    pub const fn start_reading(&mut self) {
        self.refreshing = true;
    }

    /// Takes on a fresh read, clearing whatever went wrong last time.
    pub fn replace(&mut self, dataset: Dataset, calendar: Calendar, units: Units) {
        self.reading = Some(Reading {
            dataset,
            calendar,
            units,
        });
        self.trouble = None;
        self.refreshing = false;
    }

    /// Records that a read failed, keeping the numbers already on screen.
    pub fn record_trouble(&mut self, problem: &str) {
        self.trouble = Some(problem.to_owned());
        self.refreshing = false;
    }
}
