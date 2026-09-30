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
    /// Leave this menu, and at the top level leave the shell.
    Cancel,
    /// Re-read everything on the screen.
    Refresh,
    /// Leave the session.
    Quit,
}

/// What a keystroke means.
///
/// **No ordinary key leaves the shell.** It is meant to be left running all
/// day, so Enter opens rows, Back walks out of submenus, and neither ever ends
/// the session. A bare `q` used to quit and no longer does, because `q` is one
/// keystroke away from every letter somebody presses by reflex and the widgets
/// should still be there afterwards.
///
/// Two chords leave, and they are deliberately different.
///
/// `Ctrl-C` means what it means everywhere: cancel what you are in. Inside a
/// submenu that is the submenu, so it walks out one level; at the top level
/// there is nothing left to cancel, so it ends the session. That is the shape
/// somebody's hand already knows, and it means the reflex chord never destroys
/// more than it looks like it will.
///
/// `Ctrl-Q` ends the session from anywhere, however deep the menu is. It is
/// the one key that does not care where you are.
#[must_use]
pub fn motion_for(key: KeyEvent) -> Motion {
    // Windows reports press and release; acting on both moves two rows.
    if key.kind != KeyEventKind::Press {
        return Motion::Ignore;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('q' | 'Q') => Motion::Quit,
            KeyCode::Char('c') => Motion::Cancel,
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
        KeyCode::Char('r' | 'R') => Motion::Refresh,
        KeyCode::Home | KeyCode::Char('g') => Motion::First,
        KeyCode::End | KeyCode::Char('G') => Motion::Last,
        // Counting from one on the screen, from zero in the code.
        KeyCode::Char(digit @ '1'..='9') => Motion::Highlight(digit as usize - '1' as usize),
        _ => Motion::Ignore,
    }
}
