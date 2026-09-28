//! Where the cursor is, what is typed in the filter, and what a key does.
//!
//! Pure, all of it: the interaction model (what types into the filter, what
//! moves, what leaves) is a set of tests rather than something you find out by
//! pressing keys.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// A keypress, already normalised.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// A character, which is text while the filter has focus and a shortcut
    /// otherwise.
    Char(char),
    /// Choose what is under the cursor.
    Enter,
    /// Back out: of the filter, or of the list.
    Escape,
    /// Erase a character of the filter.
    Backspace,
    /// Move up one.
    Up,
    /// Move down one.
    Down,
    /// Move up a half screen.
    PageUp,
    /// Move down a half screen.
    PageDown,
    /// Leave now, whatever has focus.
    Quit,
    /// Nothing this list acts on.
    Ignore,
}

/// What a keystroke means.
///
/// Ctrl-J and Ctrl-K move, as they do in every other list a person types at,
/// and they keep moving while the filter has focus, where `j` and `k` are
/// letters.
#[must_use]
pub const fn key_for(key: KeyEvent) -> Key {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('c' | 'd') => Key::Quit,
            KeyCode::Char('j' | 'n') => Key::Down,
            KeyCode::Char('k' | 'p') => Key::Up,
            KeyCode::Char('f') => Key::PageDown,
            KeyCode::Char('b' | 'u') => Key::PageUp,
            _ => Key::Ignore,
        };
    }
    match key.code {
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Escape,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Char(character) => Key::Char(character),
        _ => Key::Ignore,
    }
}

/// What the loop should do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    /// Draw again and wait.
    Stay,
    /// Leave, choosing nothing.
    Quit,
    /// Take what is under the cursor.
    Choose,
}

/// Where the list currently is.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct State {
    /// What is filtering the rows.
    pub query: String,
    /// Whether keystrokes are going into the filter rather than acting as
    /// shortcuts.
    pub searching: bool,
    /// The row under the cursor, as a place in the filtered list.
    pub cursor: usize,
    /// The first body line on screen.
    pub top: usize,
    /// What the last keystroke could not do.
    pub trouble: Option<String>,
}

impl State {
    /// A list showing everything, at the top.
    #[must_use]
    pub fn new(query: String) -> Self {
        Self {
            query,
            ..Self::default()
        }
    }

    /// Changes the filter and starts again from the top: the old cursor means
    /// nothing once the rows under it change.
    fn refilter(&mut self, query: String) {
        self.query = query;
        self.cursor = 0;
        self.top = 0;
    }
}

/// Where the cursor lands after a key.
///
/// Movement stops at each end rather than wrapping: a list of times reads as a
/// line, and jumping from last night to this morning because a key was held is
/// disorienting.
#[must_use]
pub fn moved(cursor: usize, rows: usize, page: usize, key: Key) -> usize {
    if rows == 0 {
        return 0;
    }
    let last = rows - 1;
    match key {
        Key::Down | Key::Char('j' | 'J' | 'h' | 'H') => (cursor + 1).min(last),
        Key::Up | Key::Char('k' | 'K' | 'p' | 'P') => cursor.saturating_sub(1),
        Key::PageDown | Key::Char('d') => (cursor + page.max(1)).min(last),
        Key::PageUp | Key::Char('u') => cursor.saturating_sub(page.max(1)),
        Key::Char('G') => last,
        Key::Char('g') => 0,
        _ => cursor.min(last),
    }
}

/// The scroll offset that keeps a line on screen.
#[must_use]
pub fn scrolled(top: usize, line: usize, height: usize, total: usize) -> usize {
    let height = height.max(1);
    let furthest = total.saturating_sub(height);
    let top = if line < top {
        line
    } else if line >= top + height {
        line + 1 - height
    } else {
        top
    };
    top.min(furthest)
}

/// Applies one keystroke.
pub fn apply(state: &mut State, key: Key, rows: usize, page: usize) -> Flow {
    state.trouble = None;
    if state.searching {
        return typing(state, key, rows, page);
    }
    match key {
        Key::Quit | Key::Escape | Key::Char('q') => Flow::Quit,
        Key::Enter => Flow::Choose,
        Key::Char('/') => {
            state.searching = true;
            Flow::Stay
        }
        Key::Up | Key::Down | Key::PageUp | Key::PageDown | Key::Char(_) => {
            state.cursor = moved(state.cursor, rows, page, key);
            Flow::Stay
        }
        Key::Backspace | Key::Ignore => Flow::Stay,
    }
}

/// Keys while the filter has focus: what is typed is text, Backspace edits it
/// and backs out once it is empty, Escape clears it.
fn typing(state: &mut State, key: Key, rows: usize, page: usize) -> Flow {
    match key {
        Key::Quit => Flow::Quit,
        Key::Enter => Flow::Choose,
        Key::Up | Key::Down | Key::PageUp | Key::PageDown => {
            state.cursor = moved(state.cursor, rows, page, key);
            Flow::Stay
        }
        Key::Escape => {
            state.searching = false;
            state.refilter(String::new());
            Flow::Stay
        }
        Key::Backspace => {
            let mut query = state.query.clone();
            if query.pop().is_none() {
                state.searching = false;
            }
            state.refilter(query);
            Flow::Stay
        }
        Key::Char(character) => {
            let mut query = state.query.clone();
            query.push(character);
            state.refilter(query);
            Flow::Stay
        }
        Key::Ignore => Flow::Stay,
    }
}

/// What the keys do, for the line at the foot of the screen.
#[must_use]
pub const fn hint(searching: bool, verb: &'static str) -> &'static str {
    if searching {
        "type to filter · ^j/^k move · enter chooses · esc clears"
    } else {
        verb
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(character: char) -> Key {
        key_for(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE))
    }

    fn control(character: char) -> Key {
        key_for(KeyEvent::new(
            KeyCode::Char(character),
            KeyModifiers::CONTROL,
        ))
    }

    #[test]
    fn the_cursor_stops_at_each_end_rather_than_wrapping() {
        assert_eq!(moved(0, 3, 5, Key::Up), 0);
        assert_eq!(moved(2, 3, 5, Key::Down), 2);
        assert_eq!(moved(0, 3, 5, Key::Down), 1);
        assert_eq!(moved(0, 0, 5, Key::Down), 0, "an empty list has nowhere");
    }

    #[test]
    fn the_ends_and_the_half_screens_are_one_keystroke_away() {
        assert_eq!(moved(0, 40, 10, press('G')), 39);
        assert_eq!(moved(39, 40, 10, press('g')), 0);
        assert_eq!(moved(0, 40, 10, press('d')), 10);
        assert_eq!(moved(20, 40, 10, press('u')), 10);
        assert_eq!(moved(0, 40, 10, Key::PageDown), 10);
    }

    #[test]
    fn the_window_follows_the_cursor_and_stops_at_the_bottom() {
        assert_eq!(scrolled(0, 5, 10, 40), 0, "already on screen");
        assert_eq!(scrolled(0, 12, 10, 40), 3);
        assert_eq!(scrolled(10, 2, 10, 40), 2, "back up to it");
        assert_eq!(scrolled(0, 39, 10, 40), 30);
        assert_eq!(scrolled(0, 3, 10, 4), 0, "a list shorter than the window");
    }

    #[test]
    fn slash_starts_a_filter_and_what_is_typed_goes_into_it() {
        let mut state = State::new(String::new());
        assert_eq!(apply(&mut state, press('/'), 3, 1), Flow::Stay);
        assert!(state.searching);
        apply(&mut state, press('d'), 3, 1);
        apply(&mut state, press('i'), 3, 1);
        assert_eq!(state.query, "di");
        assert!(!state.searching || state.query == "di");
    }

    #[test]
    fn a_letter_is_a_shortcut_until_the_filter_has_focus_and_text_after() {
        let mut state = State::new(String::new());
        state.cursor = 3;
        apply(&mut state, press('k'), 10, 1);
        assert_eq!(state.cursor, 2, "k moves");
        apply(&mut state, press('/'), 10, 1);
        apply(&mut state, press('k'), 10, 1);
        assert_eq!(state.query, "k", "and then it is a letter");
        assert_eq!(state.cursor, 0, "a new filter starts at the top");
    }

    #[test]
    fn the_arrows_move_whether_or_not_the_filter_has_focus() {
        let mut state = State::new(String::new());
        state.searching = true;
        apply(&mut state, Key::Down, 10, 1);
        assert_eq!(state.cursor, 1);
        assert!(state.searching, "and the filter keeps focus");
        apply(&mut state, control('j'), 10, 1);
        assert_eq!(state.cursor, 2);
    }

    #[test]
    fn backspace_edits_the_filter_and_backs_out_of_it_when_empty() {
        let mut state = State::new(String::new());
        apply(&mut state, press('/'), 10, 1);
        apply(&mut state, press('a'), 10, 1);
        apply(&mut state, Key::Backspace, 10, 1);
        assert_eq!(state.query, "");
        assert!(state.searching, "still typing");
        apply(&mut state, Key::Backspace, 10, 1);
        assert!(!state.searching, "and now out of it");
    }

    #[test]
    fn escape_clears_the_filter_before_it_leaves_the_list() {
        let mut state = State::new(String::new());
        apply(&mut state, press('/'), 10, 1);
        apply(&mut state, press('a'), 10, 1);
        assert_eq!(apply(&mut state, Key::Escape, 10, 1), Flow::Stay);
        assert_eq!(state.query, "");
        assert!(!state.searching);
        assert_eq!(apply(&mut state, Key::Escape, 10, 1), Flow::Quit);
    }

    #[test]
    fn three_keys_leave_rather_than_trapping_the_terminal() {
        for key in [press('q'), Key::Escape, control('c')] {
            let mut state = State::new(String::new());
            assert_eq!(apply(&mut state, key, 10, 1), Flow::Quit, "{key:?}");
        }
    }

    #[test]
    fn control_c_leaves_even_while_the_filter_has_focus() {
        let mut state = State::new(String::new());
        state.searching = true;
        assert_eq!(apply(&mut state, control('c'), 10, 1), Flow::Quit);
    }

    #[test]
    fn enter_chooses_from_either_mode() {
        let mut state = State::new(String::new());
        assert_eq!(apply(&mut state, Key::Enter, 10, 1), Flow::Choose);
        state.searching = true;
        assert_eq!(apply(&mut state, Key::Enter, 10, 1), Flow::Choose);
    }
}
