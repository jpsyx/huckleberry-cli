//! Secret input and pure key interpretation.
use crossterm::event::{KeyCode, KeyModifiers};

/// What one keystroke does while a secret is being typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keystroke {
    /// Add this character.
    Add(char),
    /// Remove the last one.
    Remove,
    /// The answer is finished.
    Submit,
    /// Abandon the command.
    Cancel,
    /// Nothing this loop understands.
    Ignore,
}

/// What a keystroke means while typing a secret.
///
/// Pure, so the two that matter are tested rather than tried: Ctrl-C
/// abandons the command instead of submitting what has been typed so far, and
/// a control character that happens to carry a letter is not that letter.
#[must_use]
pub const fn interpret_keystroke(code: KeyCode, modifiers: KeyModifiers) -> Keystroke {
    if modifiers.contains(KeyModifiers::CONTROL) {
        return match code {
            KeyCode::Char('c' | 'd') => Keystroke::Cancel,
            // Ctrl-U clears the line, as it does in a shell.
            KeyCode::Char('u') => Keystroke::Remove,
            _ => Keystroke::Ignore,
        };
    }
    match code {
        KeyCode::Enter => Keystroke::Submit,
        KeyCode::Backspace | KeyCode::Delete => Keystroke::Remove,
        KeyCode::Esc => Keystroke::Cancel,
        KeyCode::Char(character) => Keystroke::Add(character),
        _ => Keystroke::Ignore,
    }
}

/// The mask drawn in place of each character of a secret.
pub const MASK: char = '*';
