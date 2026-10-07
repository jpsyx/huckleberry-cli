//! The one-handed key map, and nothing else.
//!
//! Every motion has arrow, vim and WASD keys that mean exactly the same thing,
//! because the hand that is free at 3am is not always the one near the arrows.
//! Up and down are `k`/`w` and `j`/`s`; left and right are `h`/`a` and `l`/`d`. See
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
/// **Enter opens rows and Back walks out of submenus; neither ever ends the
/// session.** The shell is meant to be left running, so leaving it is `q`, and
/// no row in any menu offers to do it.
///
/// `q` and `Ctrl-Q` both leave, and the difference is where they reach.
/// `Ctrl-Q` ends the session from anywhere at all, including from inside a
/// question somebody is typing an answer into, where a bare `q` is a letter
/// and nothing else. See [`forces_quit`].
///
/// `Ctrl-C` means what it means everywhere: cancel what you are in. Inside a
/// submenu that is the submenu, so it walks out one level; at the top level
/// there is nothing left to cancel, so it ends the session. That is the shape
/// somebody's hand already knows, and it means the reflex chord never destroys
/// more than it looks like it will.
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
        KeyCode::Down | KeyCode::Char('j' | 'J' | 's' | 'S') => Motion::Next,
        KeyCode::Up | KeyCode::Char('k' | 'K' | 'w' | 'W') => Motion::Previous,
        KeyCode::Right | KeyCode::Char('l' | 'L' | 'd' | 'D' | ' ') | KeyCode::Enter => {
            Motion::Open
        }
        KeyCode::Left
        | KeyCode::Char('h' | 'H' | 'a' | 'A')
        | KeyCode::Esc
        | KeyCode::Backspace => Motion::Back,
        KeyCode::Char('q' | 'Q') => Motion::Quit,
        KeyCode::Char('r' | 'R') => Motion::Refresh,
        KeyCode::Home | KeyCode::Char('g') => Motion::First,
        KeyCode::End | KeyCode::Char('G') => Motion::Last,
        // Counting from one on the screen, from zero in the code.
        KeyCode::Char(digit @ '1'..='9') => Motion::Highlight(digit as usize - '1' as usize),
        _ => Motion::Ignore,
    }
}

/// Whether this key ends the session even when something else has the
/// keyboard.
///
/// Only the chord. A running command owns every other key while it is asking,
/// because a bare `q` might be a letter going into an answer, and a shell that
/// quit when somebody typed the word "quiet" would be a shell nobody trusts
/// with a text field. `Ctrl-Q` is not a letter, so it always reaches here.
#[must_use]
pub fn forces_quit(key: KeyEvent) -> bool {
    key.kind == KeyEventKind::Press
        && key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char('q' | 'Q'))
}
