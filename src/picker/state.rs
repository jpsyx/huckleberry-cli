//! What the picker is showing, and what a keystroke does to it.
//!
//! Everything here is pure: [`crate::commands::edit`] owns the terminal and
//! the keyboard, so "does the cursor stop at the last entry" is a test rather
//! than something you find out by holding a key.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::domain::log::{Entry, Kind};

/// What a keystroke asks the picker for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Nothing this loop understands.
    Ignore,
    /// Move by this many entries, positive being further down the list.
    Move(isize),
    /// Go to the first entry.
    First,
    /// Go to the last entry.
    Last,
    /// Edit the entry under the cursor.
    Take,
    /// Leave without editing anything.
    Cancel,
}

/// What a keystroke means.
///
/// `j` and `k` alongside the arrows, because this is a list in a terminal and
/// somebody's hands are already there. Three keys leave, as everywhere else in
/// this tool: trapping a terminal for guessing wrong is a bad program.
#[must_use]
pub const fn action_for(key: KeyEvent) -> Action {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('c') => Action::Cancel,
            KeyCode::Char('d') => Action::Move(10),
            KeyCode::Char('u') => Action::Move(-10),
            _ => Action::Ignore,
        };
    }
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => Action::Cancel,
        KeyCode::Char('j') | KeyCode::Down => Action::Move(1),
        KeyCode::Char('k') | KeyCode::Up => Action::Move(-1),
        KeyCode::PageDown => Action::Move(10),
        KeyCode::PageUp => Action::Move(-10),
        KeyCode::Char('g') | KeyCode::Home => Action::First,
        KeyCode::Char('G') | KeyCode::End => Action::Last,
        KeyCode::Enter => Action::Take,
        _ => Action::Ignore,
    }
}

/// The list, and where the cursor is on it.
#[derive(Debug, Clone, PartialEq)]
pub struct Picker {
    /// What there is to choose from, newest first.
    pub entries: Vec<Entry>,
    /// Which entry the cursor is on.
    pub cursor: usize,
    /// What the last keystroke could not do, for the line at the foot.
    pub trouble: Option<String>,
}

/// Whether this tool can change an entry.
///
/// It edits what it can log: a nappy, a potty trip, a bottle, a nursing
/// session, a meal and a sleep. A pumping session and a milestone are listed
/// anyway, because the stream is the stream, and saying so is better than
/// leaving somebody to wonder where their entry went.
#[must_use]
pub const fn editable(entry: &Entry) -> bool {
    entry.at.is_some() && matches!(entry.kind, Kind::Sleep | Kind::Feed | Kind::Diaper)
}

impl Picker {
    /// A picker over these entries, starting at the newest.
    #[must_use]
    pub const fn new(entries: Vec<Entry>) -> Self {
        Self {
            entries,
            cursor: 0,
            trouble: None,
        }
    }

    /// Whether there is anything to choose.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Moves the cursor, stopping at each end rather than wrapping: a list of
    /// times reads as a line, and jumping from last night to this morning
    /// because a key was held is disorienting.
    pub fn move_by(&mut self, delta: isize) {
        if self.entries.is_empty() {
            return;
        }
        let last = self.entries.len() - 1;
        let wanted = isize::try_from(self.cursor)
            .unwrap_or(0)
            .saturating_add(delta);
        self.cursor = usize::try_from(wanted).unwrap_or(0).min(last);
    }

    /// Puts the cursor on the first entry.
    pub const fn first(&mut self) {
        self.cursor = 0;
    }

    /// Puts the cursor on the last entry.
    pub fn last(&mut self) {
        self.cursor = self.entries.len().saturating_sub(1);
    }

    /// The entry under the cursor.
    #[must_use]
    pub fn selected(&self) -> Option<&Entry> {
        self.entries.get(self.cursor)
    }

    /// The entry under the cursor, if this tool can change it.
    ///
    /// Leaves a line at the foot saying why not when it cannot, rather than
    /// doing nothing at a keystroke somebody meant.
    pub fn take(&mut self) -> Option<Entry> {
        let chosen = self.selected().cloned()?;
        if editable(&chosen) {
            return Some(chosen);
        }
        self.trouble = Some(format!(
            "this tool does not log a {}, so it cannot change one",
            chosen.title.to_lowercase()
        ));
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(title: &str) -> Entry {
        Entry {
            id: title.to_owned(),
            at: Some(huckleberry_api::RowRef::loose("diaper", title)),
            kind: Kind::Diaper,
            start: 0.0,
            title: title.to_owned(),
            description: String::new(),
            notes: None,
        }
    }

    fn picker(count: usize) -> Picker {
        Picker::new((0..count).map(|index| entry(&format!("{index}"))).collect())
    }

    #[test]
    fn the_cursor_starts_on_the_newest_entry() {
        assert_eq!(picker(3).selected().expect("an entry").title, "0");
    }

    #[test]
    fn the_cursor_stops_at_each_end_rather_than_wrapping() {
        let mut picker = picker(3);
        picker.move_by(-1);
        assert_eq!(picker.cursor, 0, "already at the top");
        picker.move_by(99);
        assert_eq!(picker.cursor, 2, "and never past the bottom");
        picker.move_by(1);
        assert_eq!(picker.cursor, 2);
    }

    #[test]
    fn the_ends_are_one_keystroke_away() {
        let mut picker = picker(40);
        picker.last();
        assert_eq!(picker.cursor, 39);
        picker.first();
        assert_eq!(picker.cursor, 0);
    }

    #[test]
    fn an_empty_list_has_nothing_to_choose_and_nothing_to_move() {
        let mut picker = picker(0);
        picker.move_by(1);
        assert!(picker.is_empty());
        assert!(picker.selected().is_none());
    }

    #[test]
    fn the_keys_are_the_ones_the_dashboard_uses() {
        let press = |code| action_for(KeyEvent::new(code, KeyModifiers::NONE));
        assert_eq!(press(KeyCode::Char('j')), Action::Move(1));
        assert_eq!(press(KeyCode::Down), Action::Move(1));
        assert_eq!(press(KeyCode::Char('k')), Action::Move(-1));
        assert_eq!(press(KeyCode::Up), Action::Move(-1));
        assert_eq!(press(KeyCode::Enter), Action::Take);
    }

    #[test]
    fn three_keys_leave_rather_than_trapping_the_terminal() {
        assert_eq!(
            action_for(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            Action::Cancel
        );
        assert_eq!(
            action_for(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Action::Cancel
        );
        assert_eq!(
            action_for(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Action::Cancel
        );
    }

    #[test]
    fn an_entry_this_tool_does_not_log_is_listed_and_says_why_it_cannot_change() {
        let mut milestone = entry("Milestone");
        milestone.kind = Kind::Milestone;
        let mut picker = Picker::new(vec![milestone]);
        assert!(picker.take().is_none());
        assert!(
            picker.trouble.expect("a reason").contains("cannot change"),
            "the keystroke says why rather than doing nothing"
        );
    }

    #[test]
    fn an_entry_off_a_snapshot_cannot_be_edited_either() {
        let mut off_disk = entry("Nappy");
        off_disk.at = None;
        assert!(!editable(&off_disk));
    }

    #[test]
    fn a_key_with_no_meaning_here_is_ignored() {
        for code in [KeyCode::Tab, KeyCode::F(1), KeyCode::Char('z')] {
            assert_eq!(
                action_for(KeyEvent::new(code, KeyModifiers::NONE)),
                Action::Ignore,
                "{code:?}"
            );
        }
    }
}
