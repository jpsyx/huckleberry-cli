//! Exact instants retained independently of the dial's rounded display.
use anyhow::{Result, bail};
use crossterm::event::KeyEvent;

use super::{ControlEvent, view::navigation};
use crate::{
    domain::{Calendar, clock},
    prompt::dial::{
        self,
        model::{Dial, DialAction},
        time::{Mode, Opening},
    },
    theme::Theme,
};

/// One time control, with a supplied clock and no I/O.
pub struct TimeState {
    calendar: Calendar,
    at: f64,
    mode: Mode,
    not_before: Option<f64>,
    recorded: Option<f64>,
    floating_now: bool,
    dial: Dial,
}

impl TimeState {
    /// Opens a dial without reducing an existing instant to minute precision.
    #[must_use]
    pub fn create_time(calendar: Calendar, opening: Opening, now: f64) -> Self {
        let dial = create_dial(&calendar, opening.at, now, opening.mode, opening.not_before);
        Self {
            calendar,
            at: opening.at,
            mode: opening.mode,
            not_before: opening.not_before,
            recorded: (opening.mode == Mode::Absolute).then_some(opening.at),
            floating_now: opening.mode == Mode::Relative && opening.at.to_bits() == now.to_bits(),
            dial,
        }
    }

    /// Reads the exact instant, checking the current form's bounds.
    pub fn value(&self, now: f64) -> Result<f64> {
        let at = if self.floating_now { now } else { self.at };
        if !at.is_finite() || at < 0.0 || at > now {
            bail!("Choose a time between the Unix epoch and now");
        }
        if self.not_before.is_some_and(|start| at < start) {
            bail!("Choose a time after it began");
        }
        Ok(at)
    }

    /// Accepts the value once; later acceptance does not move it with the clock.
    pub fn accept(&mut self, now: f64) -> Result<f64> {
        self.at = self.value(now)?;
        self.floating_now = false;
        Ok(self.at)
    }

    /// Updates the lower bound without changing the retained answer.
    pub const fn set_not_before(&mut self, not_before: Option<f64>) {
        self.not_before = not_before;
    }

    /// Applies a dial key, leaving the form only from the first column.
    pub fn apply(&mut self, key: KeyEvent, now: f64) -> ControlEvent {
        if let Some(event) = navigation(key, self.dial.column == 0, true) {
            return event;
        }
        self.refresh(now);
        match self.dial.apply(key) {
            DialAction::Submit => ControlEvent::Accept,
            DialAction::Cancel => ControlEvent::Cancel,
            DialAction::Stay => ControlEvent::Stay,
            DialAction::Jumped => {
                self.at = now;
                self.floating_now = false;
                self.refresh(now);
                ControlEvent::Stay
            }
            DialAction::Switched => {
                self.mode = if self.mode == Mode::Relative {
                    Mode::Absolute
                } else {
                    Mode::Relative
                };
                self.dial = create_dial(
                    &self.calendar,
                    self.effective_at(now),
                    now,
                    self.mode,
                    self.not_before,
                );
                ControlEvent::Stay
            }
            DialAction::Turned(column) => {
                self.turned(column, now);
                ControlEvent::Stay
            }
        }
    }

    fn turned(&mut self, column: usize, now: f64) {
        self.at = match self.mode {
            Mode::Relative => {
                dial::time::settling(dial::time::furthest_back(now, self.not_before))(
                    &mut self.dial,
                    column,
                );
                now - dial::time::read_relative(&self.dial) as f64 * 60.0
            }
            Mode::Absolute => {
                let time = dial::time::read_clock(&self.dial);
                if let Some(start) = self.not_before {
                    clock::first_after(time, start, &self.calendar)
                } else if let Some(recorded) = self.recorded {
                    self.calendar
                        .at(self.calendar.day_of(recorded), time.hour, time.minute)
                } else {
                    clock::most_recent(time, now, &self.calendar)
                }
            }
        };
        self.floating_now = false;
    }

    const fn effective_at(&self, now: f64) -> f64 {
        if self.floating_now { now } else { self.at }
    }

    fn refresh(&mut self, now: f64) {
        let column = self.dial.column;
        self.dial = create_dial(
            &self.calendar,
            self.effective_at(now),
            now,
            self.mode,
            self.not_before,
        )
        .opening_on(column);
    }

    /// Draws a current relative display without changing the stored instant.
    #[must_use]
    pub fn render(&self, label: &str, now: f64, width: u16, theme: Theme) -> Vec<String> {
        let dial = create_dial(
            &self.calendar,
            self.effective_at(now),
            now,
            self.mode,
            self.not_before,
        )
        .opening_on(self.dial.column);
        let mut lines = dial::draw::render(label, &dial, width, theme);
        for line in &mut lines {
            *line = line.replace("Esc back", "Esc home");
        }
        lines
    }
}

fn create_dial(
    calendar: &Calendar,
    at: f64,
    now: f64,
    mode: Mode,
    not_before: Option<f64>,
) -> Dial {
    match mode {
        Mode::Relative => {
            let furthest = dial::time::furthest_back(now, not_before);
            dial::time::relative_dial(dial::time::ago(now, at).min(furthest), furthest)
        }
        Mode::Absolute => {
            dial::time::clock_dial(calendar.time_of_day(at), calendar.time_of_day(now))
        }
    }
}
