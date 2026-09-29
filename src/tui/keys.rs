//! The one-handed key map, and nothing else.
//!
//! Every motion has an arrow and a letter that mean exactly the same thing,
//! because the hand that is free at 3am is not always the one near the arrows.
//! Up and down are `k` and `j`; left and right are `h` and `l`. See
//! [`docs/tui.md`](../../docs/tui.md) for why that mapping is fixed.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// What a keystroke asks the shell for, before anything knows what is on
/// screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    /// Nothing this shell understands.
    Ignore,
    /// Down one row.
    Next,
    /// Up one row.
    Previous,
    /// The first row.
    First,
    /// The last row.
    Last,
    /// Put the cursor on this row, counting from zero, without opening it.
    Highlight(usize),
    /// Open, run, or accept whatever is under the cursor.
    Open,
    /// Go back to where this was opened from.
    Back,
    /// Leave the session.
    Quit,
}

/// What a keystroke means.
///
/// `q` and `Ctrl-C` leave from anywhere and Home carries an Exit row, so there
/// are always three ways out. Back is deliberately not one of them: `h` is a
/// navigation key somebody will press by reflex, and a session that ends
/// because a thumb went left one row too far is a session that gets reopened
/// in the dark.
#[must_use]
pub fn motion_for(key: KeyEvent) -> Motion {
    // Windows reports press and release; acting on both moves two rows.
    if key.kind != KeyEventKind::Press {
        return Motion::Ignore;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('c' | 'd') => Motion::Quit,
            KeyCode::Char('n') => Motion::Next,
            KeyCode::Char('p') => Motion::Previous,
            _ => Motion::Ignore,
        };
    }
    match key.code {
        KeyCode::Down | KeyCode::Char('j' | 'J') => Motion::Next,
        KeyCode::Up | KeyCode::Char('k' | 'K') => Motion::Previous,
        KeyCode::Right | KeyCode::Char('l' | 'L' | ' ') | KeyCode::Enter => Motion::Open,
        KeyCode::Left | KeyCode::Char('h' | 'H') | KeyCode::Esc | KeyCode::Backspace => {
            Motion::Back
        }
        KeyCode::Char('q' | 'Q') => Motion::Quit,
        KeyCode::Home | KeyCode::Char('g') => Motion::First,
        KeyCode::End | KeyCode::Char('G') => Motion::Last,
        // Counting from one on the screen, from zero in the code.
        KeyCode::Char(digit @ '1'..='9') => Motion::Highlight(digit as usize - '1' as usize),
        _ => Motion::Ignore,
    }
}
