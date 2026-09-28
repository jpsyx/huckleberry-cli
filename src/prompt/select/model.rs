//! Pure selection and key handling.
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// One visible menu entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem {
    /// Human-readable label.
    pub label: String,
    /// Optional secondary explanation.
    pub detail: Option<String>,
}

/// Highlight and scroll position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    /// Highlighted entry index.
    pub cursor: usize,
    /// First visible entry.
    pub top: usize,
}

/// Effect of a key on the menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionAction {
    /// Redraw and wait.
    Stay,
    /// Accept this index.
    Submit(usize),
    /// Leave without choosing.
    Cancel,
}

impl Selection {
    /// Starts on the default, bounded by the available entries.
    #[must_use]
    pub fn new(count: usize, default: usize) -> Self {
        Self {
            cursor: default.min(count.saturating_sub(1)),
            top: 0,
        }
    }

    /// Applies a key without terminal I/O.
    pub fn apply(&mut self, key: KeyEvent, count: usize, height: usize) -> SelectionAction {
        if key.kind != KeyEventKind::Press {
            return SelectionAction::Stay;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return if matches!(key.code, KeyCode::Char('c' | 'd')) {
                SelectionAction::Cancel
            } else {
                SelectionAction::Stay
            };
        }
        match key.code {
            KeyCode::Esc => return SelectionAction::Cancel,
            KeyCode::Enter if count > 0 => {
                return SelectionAction::Submit(self.cursor.min(count - 1));
            }
            KeyCode::Down | KeyCode::Char('j' | 'J' | 'h' | 'H') => {
                self.cursor = self.cursor.saturating_add(1).min(count.saturating_sub(1));
            }
            KeyCode::Up | KeyCode::Char('k' | 'K' | 'p' | 'P') => {
                self.cursor = self.cursor.saturating_sub(1);
            }
            KeyCode::Char(digit @ '1'..='9') => {
                let index = digit as usize - '1' as usize;
                if index < count {
                    self.cursor = index;
                }
            }
            _ => {}
        }
        self.top = crate::listing::state::scrolled(self.top, self.cursor, height, count);
        SelectionAction::Stay
    }
}
