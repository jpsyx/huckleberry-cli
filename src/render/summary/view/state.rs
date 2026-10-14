//! Pure cell navigation and comparison modal input.

use super::COLUMNS;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// Selection and vertical scroll, independent of terminal I/O.
#[derive(Default)]
pub struct State {
    /// Index of the selected numeric column; the day column cannot be selected.
    pub selected: usize,
    /// Selected daily row, retained independently of the column.
    pub selected_row: usize,
    /// Whether the cumulative comparison owns the keyboard.
    pub is_comparing: bool,
    pub(super) row_count: usize,
    pub(super) scroll: usize,
    pub(super) first_column: usize,
    pub(super) last_column: usize,
    pub(super) limit: usize,
    pub(super) page: usize,
}

impl State {
    /// Apply one key, returning true when the view should close.
    pub fn apply(&mut self, key: KeyEvent) -> bool {
        if key.kind == KeyEventKind::Release {
            return false;
        }
        if key.modifiers.contains(KeyModifiers::ALT) {
            return false;
        }
        if self.is_comparing {
            return self.modal_key(key);
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return matches!(key.code, KeyCode::Char('c' | 'q'));
        }
        match key.code {
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('q' | 'Q') => return true,
            KeyCode::Enter | KeyCode::Char(' ') if self.row_count > 0 => self.is_comparing = true,
            KeyCode::Down | KeyCode::Char('j' | 'J' | 's' | 'S') => {
                self.move_row(1, true);
            }
            KeyCode::Up | KeyCode::Char('k' | 'K' | 'w' | 'W') => {
                self.move_row(1, false);
            }
            KeyCode::PageDown => self.scroll = self.scroll.saturating_add(self.page),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(self.page),
            KeyCode::Char('g' | '0') => self.selected = 0,
            KeyCode::Char('G' | '$') => self.selected = COLUMNS.len() - 1,
            KeyCode::Home => {
                self.selected_row = 0;
                self.scroll = 0;
            }
            KeyCode::End => {
                self.selected_row = self.row_count.saturating_sub(1);
                self.scroll = self.limit;
            }
            _ => self.column_key(key),
        }
        self.scroll = self.scroll.min(self.limit);
        false
    }

    fn column_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Left if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.selected = self.first_column;
            }
            KeyCode::Right if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.selected = self.last_column;
            }
            KeyCode::Char('H' | 'A') => self.selected = self.first_column,
            KeyCode::Char('L' | 'D') => self.selected = self.last_column,
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h' | 'a') => {
                self.previous_column();
            }
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => self.previous_column(),
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l' | 'd') => {
                self.selected = (self.selected + 1).min(COLUMNS.len() - 1);
            }
            _ => {}
        }
    }

    fn modal_key(&mut self, key: KeyEvent) -> bool {
        if matches!(
            key.code,
            KeyCode::Esc
                | KeyCode::Backspace
                | KeyCode::Enter
                | KeyCode::Left
                | KeyCode::Char('q' | 'Q' | 'h' | 'H' | 'a' | 'A')
        ) || (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c'))
        {
            self.is_comparing = false;
        }
        key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('q')
    }

    fn move_row(&mut self, amount: usize, forward: bool) {
        if forward && self.selected_row + 1 >= self.row_count {
            self.scroll = self.scroll.saturating_add(amount);
        } else if !forward && self.scroll > self.selected_row {
            self.scroll = self.scroll.saturating_sub(amount);
        } else {
            self.selected_row = if forward {
                self.selected_row
                    .saturating_add(amount)
                    .min(self.row_count.saturating_sub(1))
            } else {
                self.selected_row.saturating_sub(amount)
            };
            self.reveal_row();
        }
    }

    pub(super) fn fit_rows(&mut self, total: usize, visible: usize) {
        let has_resized = self.page > 0 && self.page != visible.max(1);
        self.page = visible.max(1);
        if has_resized && self.scroll <= self.selected_row {
            self.reveal_row();
        }
        self.limit = total.saturating_sub(visible);
        self.scroll = self.scroll.min(self.limit);
    }

    fn reveal_row(&mut self) {
        if self.selected_row < self.scroll {
            self.scroll = self.selected_row;
        } else if self.selected_row >= self.scroll + self.page {
            self.scroll = self.selected_row + 1 - self.page.max(1);
        }
    }

    const fn previous_column(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }
}
