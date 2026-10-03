//! A row of wheels, and what keys do to them. No terminal, no clock.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::wheel::Wheel;

/// Where a dial is standing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dial {
    /// The columns, left to right.
    pub wheels: Vec<Wheel>,
    /// Which one the keys are turning.
    pub column: usize,
}

/// What a key did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialAction {
    /// Redraw and wait.
    Stay,
    /// This column moved, so there is something to animate and perhaps
    /// something for the caller to put right.
    Turned(usize),
    /// Take what it is standing on.
    Submit,
    /// Leave without it.
    Cancel,
}

impl Dial {
    /// A dial of these wheels, standing on the first column.
    #[must_use]
    pub const fn new(wheels: Vec<Wheel>) -> Self {
        Self { wheels, column: 0 }
    }

    /// What the column at `index` is standing on.
    #[must_use]
    pub fn value(&self, index: usize) -> &str {
        self.wheels.get(index).map_or("", |wheel| wheel.value())
    }

    /// Applies one key.
    ///
    /// `h` and `l` move between the columns here rather than meaning back and
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
                self.column = self.column.saturating_sub(1);
                DialAction::Stay
            }
            KeyCode::Right | KeyCode::Char('l' | 'L') => {
                self.column = (self.column + 1).min(self.wheels.len().saturating_sub(1));
                DialAction::Stay
            }
            KeyCode::Down | KeyCode::Char('j' | 'J') => self.turn(1, leaping(key)),
            KeyCode::Up | KeyCode::Char('k' | 'K') => self.turn(-1, leaping(key)),
            _ => DialAction::Stay,
        }
    }

    /// Turns the column under the cursor.
    ///
    /// One position plainly, and whatever that wheel calls a leap with shift:
    /// five minutes, five millilitres, a whole ounce. The plain turn is the
    /// small one, because the small one is the correction somebody came to
    /// make.
    fn turn(&mut self, direction: isize, leaping: bool) -> DialAction {
        let Some(wheel) = self.wheels.get_mut(self.column) else {
            return DialAction::Stay;
        };
        let step = if leaping {
            isize::try_from(wheel.leap()).unwrap_or(1)
        } else {
            1
        };
        wheel.turn(direction * step);
        DialAction::Turned(self.column)
    }

    /// The dials passed through on the way here from `before`.
    ///
    /// Excludes both ends: these are the frames an animation draws, and the
    /// one it lands on is drawn by the loop that was going to draw it anyway.
    #[must_use]
    pub fn passed_through(&self, before: &Self) -> Vec<Self> {
        let Some((column, wheel)) = self.wheels.iter().enumerate().find(|(index, wheel)| {
            before
                .wheels
                .get(*index)
                .is_some_and(|was| was.index() != wheel.index() || was.count() != wheel.count())
        }) else {
            return Vec::new();
        };
        let Some(was) = before.wheels.get(column) else {
            return Vec::new();
        };
        // A wheel that was rebuilt under the value has no path to walk: the
        // positions either side of it are not the ones it passed.
        if was.count() != wheel.count() {
            return Vec::new();
        }
        let travelled = wheel.shortest_way(was.index());
        if travelled.abs() <= 1 {
            return Vec::new();
        }
        let direction = travelled.signum();
        (1..travelled.abs())
            .map(|step| {
                let mut passing = self.clone();
                if let Some(turning) = passing.wheels.get_mut(column) {
                    *turning = was.clone();
                    turning.turn(direction * step);
                }
                passing
            })
            .collect()
    }
}

/// Whether this key asks for the big step.
///
/// Shift and an uppercase letter are the same press: a terminal may report
/// either, depending on how it was built.
const fn leaping(key: KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::SHIFT) || matches!(key.code, KeyCode::Char('J' | 'K'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbers(count: usize, leap: usize) -> Wheel {
        Wheel::new(
            (0..count).map(|value| value.to_string()).collect(),
            0,
            leap,
            "",
        )
    }

    fn dial() -> Dial {
        Dial::new(vec![numbers(12, 1), numbers(60, 5)])
    }

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn shifted(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::SHIFT)
    }

    #[test]
    fn left_and_right_move_between_the_columns_and_the_ends_hold() {
        let mut dial = dial();
        for key in [KeyCode::Right, KeyCode::Char('l')] {
            dial.column = 0;
            dial.apply(press(key));
            assert_eq!(dial.column, 1, "{key:?}");
        }
        dial.apply(press(KeyCode::Right));
        assert_eq!(dial.column, 1, "and holds at the end");
        for key in [KeyCode::Left, KeyCode::Char('h')] {
            dial.column = 1;
            dial.apply(press(key));
            assert_eq!(dial.column, 0, "{key:?}");
        }
        dial.apply(press(KeyCode::Left));
        assert_eq!(dial.column, 0, "and holds at that end too");
    }

    /// The small step is the plain one: the correction somebody came to make
    /// is usually a nudge, and the big jump is the one worth reaching for.
    #[test]
    fn a_plain_turn_moves_one_and_a_shifted_turn_leaps() {
        let mut dial = dial();
        dial.column = 1;
        dial.apply(press(KeyCode::Down));
        assert_eq!(dial.value(1), "1");
        dial.apply(shifted(KeyCode::Down));
        assert_eq!(dial.value(1), "6");
        dial.apply(shifted(KeyCode::Up));
        assert_eq!(dial.value(1), "1");
        dial.apply(press(KeyCode::Up));
        assert_eq!(dial.value(1), "0");
    }

    #[test]
    fn an_uppercase_letter_leaps_the_same_way_shift_does() {
        let mut dial = dial();
        dial.column = 1;
        dial.apply(KeyEvent::new(KeyCode::Char('J'), KeyModifiers::SHIFT));
        assert_eq!(dial.value(1), "5");
        dial.apply(KeyEvent::new(KeyCode::Char('K'), KeyModifiers::SHIFT));
        assert_eq!(dial.value(1), "0");
    }

    #[test]
    fn turning_one_column_never_touches_another() {
        let mut dial = dial();
        dial.column = 1;
        for _ in 0..70 {
            dial.apply(press(KeyCode::Down));
        }
        assert_eq!(dial.value(0), "0", "the hours did not follow the minutes");
        assert_eq!(dial.value(1), "10", "which wrapped on their own");
    }

    #[test]
    fn enter_takes_it_and_escape_leaves_without_it() {
        let mut dial = dial();
        assert_eq!(dial.apply(press(KeyCode::Enter)), DialAction::Submit);
        assert_eq!(dial.apply(press(KeyCode::Esc)), DialAction::Cancel);
        assert_eq!(
            dial.apply(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            DialAction::Cancel
        );
    }

    #[test]
    fn a_turn_says_which_column_moved_and_a_dead_key_says_nothing() {
        let mut dial = dial();
        dial.column = 1;
        assert_eq!(dial.apply(press(KeyCode::Down)), DialAction::Turned(1));
        assert_eq!(dial.apply(press(KeyCode::Char('z'))), DialAction::Stay);
        assert_eq!(dial.apply(press(KeyCode::Left)), DialAction::Stay);
    }

    /// A leap is drawn as the places it crossed, so it can be seen to travel
    /// and in which direction.
    #[test]
    fn a_leap_passes_through_everything_between() {
        let before = {
            let mut dial = dial();
            dial.column = 1;
            dial
        };
        let mut after = before.clone();
        after.apply(shifted(KeyCode::Down));
        let seen: Vec<String> = after
            .passed_through(&before)
            .iter()
            .map(|dial| dial.value(1).to_owned())
            .collect();
        assert_eq!(seen, ["1", "2", "3", "4"]);
    }

    #[test]
    fn a_leap_the_other_way_passes_through_them_backwards_and_wraps() {
        let before = {
            let mut dial = dial();
            dial.column = 1;
            dial
        };
        let mut after = before.clone();
        after.apply(shifted(KeyCode::Up));
        let seen: Vec<String> = after
            .passed_through(&before)
            .iter()
            .map(|dial| dial.value(1).to_owned())
            .collect();
        assert_eq!(seen, ["59", "58", "57", "56"]);
    }

    #[test]
    fn a_single_step_passes_through_nothing() {
        let before = dial();
        let mut after = before.clone();
        after.apply(press(KeyCode::Down));
        assert!(after.passed_through(&before).is_empty());
    }

    /// A column rebuilt under the value did not travel: the positions either
    /// side of it are not the ones it passed.
    #[test]
    fn a_rebuilt_column_has_no_path_to_animate() {
        let before = dial();
        let mut after = before.clone();
        after.wheels[1] = numbers(8, 4);
        after.wheels[1].turn(3);
        assert!(after.passed_through(&before).is_empty());
    }
}
