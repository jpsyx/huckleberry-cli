//! Shared prompts: finite choices are menus, free values use a text editor.
pub mod dial;
pub mod host;
mod question;
mod secret;
pub mod select;
pub mod terminal;
mod text;
pub mod time;

use crate::theme::Theme;
use anyhow::{Result, bail};
pub use question::{Choice, Question, Reply, interpret, render, unanswerable_message};
pub use secret::{Keystroke, MASK, interpret_keystroke};

/// A cancelled question, distinct from an I/O or validation failure.
#[derive(Debug)]
pub struct Cancelled;

impl std::fmt::Display for Cancelled {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("cancelled")
    }
}
impl std::error::Error for Cancelled {}

/// Recognizes cancellation even after contextual error wrapping.
#[must_use]
pub fn is_cancelled(error: &anyhow::Error) -> bool {
    error.is::<Cancelled>()
}

/// Whether both input and prompt output support interaction.
#[must_use]
pub fn available() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal() && std::io::stderr().is_terminal()
}

/// Asks a required question; explicit-command callers retain flag errors on pipes.
pub fn ask(question: &Question<'_>, theme: Theme) -> Result<String> {
    if !available() {
        bail!(unanswerable_message(question));
    }
    if !question.choices.is_empty() {
        return select::answer(question, theme)?.ok_or_else(|| Cancelled.into());
    }
    loop {
        let answer = text::read(question, theme, false)?;
        match interpret(&answer, &[], question.default) {
            Reply::Accepted(value) => return Ok(value),
            _ => crate::render::note(&theme.warning("An answer is required.")),
        }
    }
}

/// What an empty optional field means, said on the field itself.
///
/// An empty box gives no clue that leaving it empty is allowed, and the menu
/// this replaced said so in a row of its own.
const SKIP_HINT: &str = "Enter to skip";

/// Asks an optional value.
///
/// In the shell, a question with nothing to choose between is shown as the
/// field itself: a menu whose two rows read "Skip" and "Enter text" is a menu
/// with nothing to choose, and at 3am it is one keypress in the way of the
/// answer. Leaving the field empty is the skip, which is what an empty answer
/// means everywhere else in this tool.
///
/// On a bare terminal the menu stays, because there the rows are also how
/// somebody discovers that skipping is an option at all.
pub fn ask_optional(question: &Question<'_>, theme: Theme) -> Result<Option<String>> {
    if host::hosted() && question.choices.is_empty() {
        let asked = if question.help.is_some() {
            *question
        } else {
            question.with_help(SKIP_HINT)
        };
        let typed = text::read(&asked, theme, false)?;
        return Ok((!typed.trim().is_empty()).then(|| typed.trim().to_owned()));
    }
    if !available() {
        return Ok(None);
    }
    select::answer(&question.optional(), theme)
}

/// Reads a secret without showing its contents.
pub fn ask_secret(question: &Question<'_>, theme: Theme) -> Result<String> {
    if !available() {
        bail!(unanswerable_message(question));
    }
    let answer = text::read(question, theme, true)?;
    if answer.trim().is_empty() {
        bail!("no {}: nothing was typed", question.subject);
    }
    Ok(answer)
}

/// A Yes/No menu whose highlighted answer is the existing default.
pub fn confirm(label: &str, default: bool, theme: Theme) -> Result<bool> {
    if !available() {
        return Ok(default);
    }
    let choices = [
        Choice {
            value: "yes",
            hint: "Yes",
        },
        Choice {
            value: "no",
            hint: "No",
        },
    ];
    let question = Question::new("confirmation", label, "--yes")
        .with_choices(&choices)
        .with_default(if default { "yes" } else { "no" });
    Ok(ask(&question, theme)? == "yes")
}

/// Interprets legacy line confirmations for noninteractive callers.
#[must_use]
pub fn interpret_confirmation(input: &str, default: bool) -> bool {
    match input.trim().to_lowercase().as_str() {
        "y" | "yes" => true,
        "n" | "no" => false,
        _ => default,
    }
}

#[cfg(test)]
mod tests;
