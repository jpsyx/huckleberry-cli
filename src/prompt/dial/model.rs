//! A row of wheels, and what keys do to them. No terminal, no clock.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::wheel::Wheel;

/// A value worth pointing out, and what to call it.
///
/// One value per column, and the mark goes on the row where **the column
/// being turned** is showing its own. A dial is a row of columns that turn
/// separately, so there is usually no row that reads the whole landmark; the
/// question the mark answers is "where do I turn this back to", and that is
/// a question about one column at a time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Landmark {
    /// What each column reads when the row is the one to mark.
    pub values: Vec<String>,
    /// What to write beside it.
    pub label: String,
}

/// Where a dial is standing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dial {
    /// The columns, left to right.
    pub wheels: Vec<Wheel>,
    /// Which one the keys are turning.
    pub column: usize,
    /// A row to point out, if there is one.
    pub landmark: Option<Landmark>,
    /// What Tab switches to, for a dial that has another way of saying the
    /// same thing. `None` means Tab does nothing.
    pub switch: Option<String>,
    /// A word after the columns on the row that is the answer.
    ///
    /// Two numbers with a letter each is not a sentence: "1h 06m" says which
    /// numbers they are and not what they mean. Drawn on the middle row
    /// alone, because that is the row the word is true of.
    pub trailing: Option<String>,
}

/// What a key did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialAction {
    /// Redraw and wait.
    Stay,
    /// This column moved, so there is something to animate and perhaps
    /// something for the caller to put right.
    Turned(usize),
    /// Every column went back to the landmark at once.
    ///
    /// Not a turn: no single column moved, so there is nothing to settle
    /// against, and nothing travelled a path worth watching on the way.
    Jumped,
    /// Say it the other way instead. The dial itself is replaced.
    Switched,
    /// Take what it is standing on.
    Submit,
    /// Leave without it.
    Cancel,
}

impl Dial {
    /// A dial of these wheels, standing on the first column.
    #[must_use]
    pub const fn new(wheels: Vec<Wheel>) -> Self {
        Self {
            wheels,
            column: 0,
            landmark: None,
            switch: None,
            trailing: None,
        }
    }

    /// The same dial, with a word after its columns on the answering row.
    #[must_use]
    pub fn reading(mut self, trailing: &str) -> Self {
        self.trailing = Some(trailing.to_owned());
        self
    }

    /// The same dial, opened with the cursor on a given column.
    ///
    /// Clamped to the columns there are, so a dial can name the one it wants
    /// without knowing how many it ended up with.
    #[must_use]
    pub fn opening_on(mut self, column: usize) -> Self {
        self.column = column.min(self.wheels.len().saturating_sub(1));
        self
    }

    /// The same dial, with another way of saying the same thing behind Tab.
    ///
    /// `hint` names what Tab leads to, so the key is advertised by its
    /// destination rather than by itself.
    #[must_use]
    pub fn switching_to(mut self, hint: &str) -> Self {
        self.switch = Some(hint.to_owned());
        self
    }

    /// The same dial, with a row worth pointing out.
    #[must_use]
    pub fn marking(mut self, label: &str, values: Vec<String>) -> Self {
        self.landmark = Some(Landmark {
            values,
            label: label.to_owned(),
        });
        self
    }

    /// Whether the row `offset` places from the middle carries the mark.
    ///
    /// Asked of the column being turned and no other, so the mark moves with
    /// the cursor and points at the row that puts this column back. Matched
    /// on what that column actually draws on that row, so a column of two is
    /// marked where its other value is drawn rather than where a wheel of
    /// sixty would have put it.
    #[must_use]
    pub fn is_landmark(&self, offset: isize) -> bool {
        let Some(landmark) = &self.landmark else {
            return false;
        };
        let (Some(wheel), Some(wanted)) = (
            self.wheels.get(self.column),
            landmark.values.get(self.column),
        ) else {
            return false;
        };
        !wanted.is_empty() && wheel.shown_at(offset) == wanted
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
            // `n` for now, which is the only landmark any dial has. If one
            // ever gets another kind, the key belongs in the landmark rather
            // than here.
            KeyCode::Tab | KeyCode::BackTab if self.switch.is_some() => DialAction::Switched,
            KeyCode::Char('n' | 'N') => self.jump(),
            KeyCode::Down | KeyCode::Char('j' | 'J') => self.turn(1, leaping(key)),
            KeyCode::Up | KeyCode::Char('k' | 'K') => self.turn(-1, leaping(key)),
            _ => DialAction::Stay,
        }
    }

    /// Puts every column back on the landmark.
    ///
    /// One key, because turning back by hand is a column at a time and the
    /// dial is most often wanted exactly where it opened.
    fn jump(&mut self) -> DialAction {
        let Some(landmark) = self.landmark.clone() else {
            return DialAction::Stay;
        };
        if landmark.values.len() != self.wheels.len() {
            return DialAction::Stay;
        }
        let moved = self
            .wheels
            .iter_mut()
            .zip(&landmark.values)
            .fold(false, |moved, (wheel, wanted)| {
                wheel.stand_on(wanted) || moved
            });
        if moved {
            DialAction::Jumped
        } else {
            DialAction::Stay
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

    /// The dial opens on now and gets turned away from it. Turning back by
    /// hand is a column at a time; one key is the way back.
    #[test]
    fn pressing_n_puts_every_column_back_on_the_landmark() {
        let mut dial = marked();
        dial.column = 1;
        for _ in 0..7 {
            dial.apply(press(KeyCode::Down));
        }
        dial.column = 0;
        dial.apply(press(KeyCode::Down));
        assert!(!dial.is_landmark(0), "well away from it now");

        assert_eq!(dial.apply(press(KeyCode::Char('n'))), DialAction::Jumped);
        assert!(dial.is_landmark(0), "and back on it in one key");
    }

    #[test]
    fn pressing_n_puts_a_column_of_two_back_as_well() {
        let mut dial = marked();
        dial.column = 2;
        dial.apply(press(KeyCode::Down));
        assert!(!dial.is_landmark(0));
        dial.apply(press(KeyCode::Char('n')));
        assert!(dial.is_landmark(0));
    }

    /// A jump is a reset rather than a turn, and says so: the loops animate
    /// a turn and redraw a jump. See the loop's own test for that half.
    #[test]
    fn a_jump_is_reported_as_one_rather_than_as_a_turn() {
        let mut dial = marked();
        dial.column = 1;
        for _ in 0..9 {
            dial.apply(press(KeyCode::Down));
        }
        assert_eq!(dial.apply(press(KeyCode::Char('n'))), DialAction::Jumped);
    }

    #[test]
    fn n_on_a_dial_with_nowhere_to_jump_does_nothing() {
        let mut dial = dial();
        assert_eq!(dial.apply(press(KeyCode::Char('n'))), DialAction::Stay);
    }

    /// Already there is not a move, so the screen has no reason to redraw.
    #[test]
    fn n_when_it_is_already_on_the_landmark_is_not_a_move() {
        let mut dial = marked();
        assert_eq!(dial.apply(press(KeyCode::Char('n'))), DialAction::Stay);
    }

    fn marked() -> Dial {
        Dial::new(vec![numbers(12, 1), numbers(60, 5), two()])
            .marking("[now]", vec!["0".into(), "0".into(), "am".into()])
    }

    fn two() -> Wheel {
        Wheel::new(vec!["am".into(), "pm".into()], 0, 1, "   ")
    }

    /// The mark follows the column being turned, and says where that
    /// column's present value sits. That is the row to turn back to, which
    /// is the only question the mark is there to answer.
    #[test]
    fn the_mark_sits_on_the_present_value_of_the_column_being_turned() {
        let dial = marked();
        assert!(dial.is_landmark(0), "it opened standing on it");
        for offset in [-2, -1, 1, 2] {
            assert!(!dial.is_landmark(offset), "offset {offset}");
        }
    }

    #[test]
    fn turning_away_moves_the_mark_to_the_row_that_value_is_now_on() {
        let mut dial = marked();
        dial.apply(press(KeyCode::Down));
        assert!(dial.is_landmark(-1), "one row back up");
        assert!(!dial.is_landmark(0));
        dial.apply(press(KeyCode::Down));
        assert!(dial.is_landmark(-2), "two rows back up");
    }

    /// Out of sight is unmarked: the mark never points off the screen.
    #[test]
    fn nothing_is_marked_once_that_value_has_scrolled_out_of_sight() {
        let mut dial = marked();
        for _ in 0..3 {
            dial.apply(press(KeyCode::Down));
        }
        for offset in -2..=2 {
            assert!(!dial.is_landmark(offset), "offset {offset}");
        }
    }

    /// Moving between columns moves the mark, because it is about whichever
    /// column the keys are turning.
    #[test]
    fn the_mark_follows_the_cursor_between_columns() {
        let mut dial = marked();
        dial.apply(press(KeyCode::Down));
        assert!(dial.is_landmark(-1), "the hour is one row off");
        dial.apply(press(KeyCode::Right));
        assert!(
            dial.is_landmark(0),
            "the minutes never moved, so theirs is the middle row"
        );
    }

    /// A column of two shows the other value directly above, so that is
    /// where its mark goes once it has been toggled.
    #[test]
    fn a_column_of_two_is_marked_where_it_is_drawn() {
        let mut dial = marked();
        dial.column = 2;
        assert!(dial.is_landmark(0));
        dial.apply(press(KeyCode::Down));
        assert!(dial.is_landmark(-1), "the row the other one is drawn on");
        assert!(!dial.is_landmark(0));
    }

    #[test]
    fn a_dial_with_nothing_to_point_out_marks_no_row() {
        let dial = Dial::new(vec![numbers(12, 1)]);
        for offset in -2..=2 {
            assert!(!dial.is_landmark(offset));
        }
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
