//! The three dials, and what keys do to them. No terminal, no clock.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::domain::clock::TimeOfDay;

/// How far one press moves the minutes.
///
/// Five, because a feed logged at 1:40 and one logged at 1:42 are the same
/// feed, and a dial that needs twelve presses to cross an hour is a dial
/// somebody types around instead.
pub const MINUTE_STEP: i16 = 5;

/// Which dial the keys are turning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    /// Hours, 1 to 12.
    Hour,
    /// Minutes, 0 to 59.
    Minute,
    /// Morning or afternoon.
    Meridiem,
}

impl Column {
    /// Every column, left to right, which is also the order they are drawn.
    pub const ALL: [Self; 3] = [Self::Hour, Self::Minute, Self::Meridiem];
}

/// Morning or afternoon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Meridiem {
    /// Before noon.
    Am,
    /// After it.
    Pm,
}

impl Meridiem {
    /// The other one. This dial has no third state, so turning it either way
    /// does the same thing.
    #[must_use]
    pub const fn flipped(self) -> Self {
        match self {
            Self::Am => Self::Pm,
            Self::Pm => Self::Am,
        }
    }

    /// How it reads: `am`, `pm`.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Am => "am",
            Self::Pm => "pm",
        }
    }
}

/// Where the three dials are standing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dial {
    /// The hour as it is shown, 1 to 12.
    pub hour: i16,
    /// The minute, 0 to 59.
    pub minute: i16,
    /// Which half of the day.
    pub meridiem: Meridiem,
    /// Which dial the keys are turning.
    pub column: Column,
}

/// What a key did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialAction {
    /// Redraw and wait.
    Stay,
    /// A dial moved, so there is something to animate.
    Turned,
    /// Take this time.
    Submit,
    /// Leave without one.
    Cancel,
}

impl Dial {
    /// The dial standing at a time, with the hour under the cursor.
    #[must_use]
    pub fn new(time: TimeOfDay) -> Self {
        let (hour, meridiem) = match i16::from(time.hour) {
            0 => (12, Meridiem::Am),
            12 => (12, Meridiem::Pm),
            hour if hour > 12 => (hour - 12, Meridiem::Pm),
            hour => (hour, Meridiem::Am),
        };
        Self {
            hour,
            minute: i16::from(time.minute),
            meridiem,
            column: Column::Hour,
        }
    }

    /// The time it is standing at.
    #[must_use]
    pub fn time(&self) -> TimeOfDay {
        let hour = match (self.hour, self.meridiem) {
            (12, Meridiem::Am) => 0,
            (12, Meridiem::Pm) => 12,
            (hour, Meridiem::Pm) => hour + 12,
            (hour, Meridiem::Am) => hour,
        };
        TimeOfDay {
            hour: i8::try_from(hour).unwrap_or(0),
            minute: i8::try_from(self.minute).unwrap_or(0),
        }
    }

    /// The hour `offset` places along the dial, wrapping past twelve.
    #[must_use]
    pub const fn hour_at(&self, offset: i16) -> i16 {
        wrapped(self.hour - 1 + offset, 12) + 1
    }

    /// The minute `offset` places along the dial, wrapping past fifty-nine.
    #[must_use]
    pub const fn minute_at(&self, offset: i16) -> i16 {
        wrapped(self.minute + offset, 60)
    }

    /// Applies one key.
    ///
    /// `h` and `l` move between the dials here rather than meaning back and
    /// forward, which is the one place in this tool they do. A dial is a row
    /// of columns and there is nowhere else for those keys to point; Esc is
    /// how somebody leaves, and the hint line says so.
    pub fn apply(&mut self, key: KeyEvent) -> DialAction {
        if key.kind != KeyEventKind::Press {
            return DialAction::Stay;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return if matches!(key.code, KeyCode::Char('c' | 'd')) {
                DialAction::Cancel
            } else {
                DialAction::Stay
            };
        }
        match key.code {
            KeyCode::Esc => DialAction::Cancel,
            KeyCode::Enter => DialAction::Submit,
            KeyCode::Left | KeyCode::Char('h' | 'H') => {
                self.step_column(-1);
                DialAction::Stay
            }
            KeyCode::Right | KeyCode::Char('l' | 'L') => {
                self.step_column(1);
                DialAction::Stay
            }
            KeyCode::Down | KeyCode::Char('j' | 'J') => self.turn(1, by_one(key)),
            KeyCode::Up | KeyCode::Char('k' | 'K') => self.turn(-1, by_one(key)),
            _ => DialAction::Stay,
        }
    }

    /// Turns the dial under the cursor one notch in `direction`.
    const fn turn(&mut self, direction: i16, by_one: bool) -> DialAction {
        match self.column {
            Column::Hour => self.hour = self.hour_at(direction),
            Column::Minute => {
                let step = if by_one { 1 } else { MINUTE_STEP };
                self.minute = self.minute_at(direction * step);
            }
            // Never on its own, and never a third state: whichever way it is
            // turned, there is only the other one to reach.
            Column::Meridiem => self.meridiem = self.meridiem.flipped(),
        }
        DialAction::Turned
    }

    /// Moves the cursor along the row, holding at either end.
    fn step_column(&mut self, delta: isize) {
        let index = Column::ALL
            .iter()
            .position(|column| *column == self.column)
            .unwrap_or_default();
        let moved = index
            .saturating_add_signed(delta)
            .min(Column::ALL.len() - 1);
        self.column = Column::ALL[moved];
    }

    /// The dials passed through on the way here from `before`.
    ///
    /// Excludes both ends: these are the frames an animation draws, and the
    /// one it lands on is drawn by the loop that was going to draw it anyway.
    #[must_use]
    pub fn passed_through(self, before: Self) -> Vec<Self> {
        if self.hour != before.hour || self.meridiem != before.meridiem {
            return Vec::new();
        }
        let travelled = shortest_way(before.minute, self.minute);
        if travelled.abs() <= 1 {
            return Vec::new();
        }
        let direction = travelled.signum();
        (1..travelled.abs())
            .map(|step| Self {
                minute: before.minute_at(direction * step),
                ..self
            })
            .collect()
    }
}

/// Whether this key asks for the small step.
///
/// Shift and an uppercase letter are the same press: a terminal may report
/// either, depending on how it was built.
const fn by_one(key: KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::SHIFT) || matches!(key.code, KeyCode::Char('J' | 'K'))
}

/// How far it is from one minute to another, the short way round.
///
/// Signed, so crossing the hour reads as five minutes forward rather than
/// fifty-five back, which is what it looked like to the parent turning it.
const fn shortest_way(from: i16, to: i16) -> i16 {
    let forward = wrapped(to - from, 60);
    if forward > 30 { forward - 60 } else { forward }
}

/// `value` brought back inside `0..modulus`, for negatives too.
const fn wrapped(value: i16, modulus: i16) -> i16 {
    ((value % modulus) + modulus) % modulus
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hour: i8, minute: i8) -> Dial {
        Dial::new(TimeOfDay::new(hour, minute).expect("a time"))
    }

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn shifted(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::SHIFT)
    }

    #[test]
    fn a_time_becomes_a_twelve_hour_dial_and_comes_back_unchanged() {
        for hour in 0..24 {
            for minute in [0, 7, 59] {
                let time = TimeOfDay::new(hour, minute).expect("a time");
                assert_eq!(Dial::new(time).time(), time, "{hour}:{minute}");
            }
        }
    }

    #[test]
    fn midnight_and_noon_are_both_twelve_on_the_dial() {
        assert_eq!(at(0, 0).hour, 12);
        assert_eq!(at(0, 0).meridiem, Meridiem::Am);
        assert_eq!(at(12, 0).hour, 12);
        assert_eq!(at(12, 0).meridiem, Meridiem::Pm);
    }

    #[test]
    fn the_hour_cycles_so_there_is_always_one_above_and_below() {
        let dial = at(12, 0);
        assert_eq!(dial.hour_at(1), 1, "after twelve comes one");
        assert_eq!(dial.hour_at(2), 2);
        assert_eq!(at(1, 0).hour_at(-1), 12, "and before one comes twelve");
    }

    #[test]
    fn the_minute_cycles_the_same_way() {
        let dial = at(1, 59);
        assert_eq!(dial.minute_at(1), 0);
        assert_eq!(dial.minute_at(2), 1);
        assert_eq!(at(1, 0).minute_at(-1), 59);
    }

    #[test]
    fn left_and_right_move_between_the_three_dials() {
        let mut dial = at(1, 0);
        assert_eq!(dial.column, Column::Hour);
        for key in [KeyCode::Right, KeyCode::Char('l')] {
            dial.column = Column::Hour;
            dial.apply(press(key));
            assert_eq!(dial.column, Column::Minute, "{key:?}");
        }
        dial.apply(press(KeyCode::Right));
        assert_eq!(dial.column, Column::Meridiem);
        for key in [KeyCode::Left, KeyCode::Char('h')] {
            dial.column = Column::Meridiem;
            dial.apply(press(key));
            assert_eq!(dial.column, Column::Minute, "{key:?}");
        }
    }

    /// The ends hold rather than wrap: a dial that jumps from the hour to the
    /// meridiem when you overshoot is one you have to look at to use.
    #[test]
    fn the_outer_dials_hold_rather_than_wrapping_round() {
        let mut dial = at(1, 0);
        dial.apply(press(KeyCode::Left));
        assert_eq!(dial.column, Column::Hour);
        dial.column = Column::Meridiem;
        dial.apply(press(KeyCode::Right));
        assert_eq!(dial.column, Column::Meridiem);
    }

    #[test]
    fn the_hour_turns_one_at_a_time_in_both_directions() {
        let mut dial = at(3, 0);
        dial.apply(press(KeyCode::Down));
        assert_eq!(dial.hour, 4);
        dial.apply(press(KeyCode::Char('j')));
        assert_eq!(dial.hour, 5);
        dial.apply(press(KeyCode::Up));
        assert_eq!(dial.hour, 4);
        dial.apply(press(KeyCode::Char('k')));
        assert_eq!(dial.hour, 3);
    }

    #[test]
    fn the_minute_turns_five_at_a_time() {
        let mut dial = at(1, 44);
        dial.column = Column::Minute;
        dial.apply(press(KeyCode::Down));
        assert_eq!(dial.minute, 49);
        dial.apply(press(KeyCode::Up));
        assert_eq!(dial.minute, 44);
    }

    #[test]
    fn the_minute_turns_one_at_a_time_with_shift() {
        let mut dial = at(1, 44);
        dial.column = Column::Minute;
        dial.apply(shifted(KeyCode::Down));
        assert_eq!(dial.minute, 45);
        dial.apply(shifted(KeyCode::Up));
        assert_eq!(dial.minute, 44);
        dial.apply(KeyEvent::new(KeyCode::Char('J'), KeyModifiers::SHIFT));
        assert_eq!(dial.minute, 45, "shift+j is the same key");
        dial.apply(KeyEvent::new(KeyCode::Char('K'), KeyModifiers::SHIFT));
        assert_eq!(dial.minute, 44);
    }

    #[test]
    fn the_minute_wraps_past_the_hour_without_moving_it() {
        let mut dial = at(1, 57);
        dial.column = Column::Minute;
        dial.apply(press(KeyCode::Down));
        assert_eq!(
            (dial.minute, dial.hour),
            (2, 1),
            "the hour is a separate dial"
        );
    }

    /// Either direction does the same thing, and it never moves on its own.
    #[test]
    fn the_meridiem_is_a_toggle_whichever_way_it_is_turned() {
        let mut dial = at(1, 0);
        dial.column = Column::Meridiem;
        assert_eq!(dial.meridiem, Meridiem::Am);
        dial.apply(press(KeyCode::Down));
        assert_eq!(dial.meridiem, Meridiem::Pm);
        dial.apply(press(KeyCode::Down));
        assert_eq!(dial.meridiem, Meridiem::Am);
        dial.apply(press(KeyCode::Up));
        assert_eq!(dial.meridiem, Meridiem::Pm);
    }

    #[test]
    fn turning_the_hour_past_twelve_never_touches_the_meridiem() {
        let mut dial = at(11, 0);
        assert_eq!(dial.meridiem, Meridiem::Am);
        for _ in 0..3 {
            dial.apply(press(KeyCode::Down));
        }
        assert_eq!((dial.hour, dial.meridiem), (2, Meridiem::Am));
    }

    #[test]
    fn enter_takes_the_time_and_escape_leaves_without_one() {
        let mut dial = at(1, 0);
        assert_eq!(dial.apply(press(KeyCode::Enter)), DialAction::Submit);
        assert_eq!(dial.apply(press(KeyCode::Esc)), DialAction::Cancel);
        assert_eq!(
            dial.apply(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            DialAction::Cancel
        );
    }

    #[test]
    fn turning_a_dial_says_so_and_pressing_nothing_useful_does_not() {
        let mut dial = at(1, 0);
        assert_eq!(dial.apply(press(KeyCode::Down)), DialAction::Turned);
        assert_eq!(dial.apply(press(KeyCode::Char('z'))), DialAction::Stay);
        assert_eq!(dial.apply(press(KeyCode::Right)), DialAction::Stay);
    }

    /// A five-minute turn passes four minutes on the way, so it can be seen
    /// to travel rather than appearing to jump.
    #[test]
    fn a_five_minute_turn_passes_through_the_minutes_between() {
        let before = {
            let mut dial = at(1, 44);
            dial.column = Column::Minute;
            dial
        };
        let mut after = before;
        after.apply(press(KeyCode::Down));
        let minutes: Vec<i16> = after
            .passed_through(before)
            .iter()
            .map(|dial| dial.minute)
            .collect();
        assert_eq!(minutes, vec![45, 46, 47, 48]);
    }

    #[test]
    fn a_turn_going_the_other_way_passes_through_them_backwards() {
        let before = {
            let mut dial = at(1, 2);
            dial.column = Column::Minute;
            dial
        };
        let mut after = before;
        after.apply(press(KeyCode::Up));
        let minutes: Vec<i16> = after
            .passed_through(before)
            .iter()
            .map(|dial| dial.minute)
            .collect();
        assert_eq!(minutes, vec![1, 0, 59, 58], "and wraps while it does it");
    }

    /// One step has nothing in between, so there is nothing to animate.
    #[test]
    fn a_single_step_passes_through_nothing() {
        let before = at(3, 0);
        let mut after = before;
        after.apply(press(KeyCode::Down));
        assert!(after.passed_through(before).is_empty());

        let before = {
            let mut dial = at(1, 44);
            dial.column = Column::Minute;
            dial
        };
        let mut after = before;
        after.apply(shifted(KeyCode::Down));
        assert!(after.passed_through(before).is_empty());
    }
}
